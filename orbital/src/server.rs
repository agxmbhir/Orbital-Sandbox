use std::sync::Mutex;
use std::collections::HashMap;
use actix_cors::Cors;
use actix_web::{ get, post, web, App, HttpResponse, HttpServer, Responder, middleware::Logger };
use serde::{ Deserialize, Serialize };
use crate::ticks::MultiTickAMM;
use actix_files as fs;

// Helper function to safely get AMM from poisoned mutex
fn get_amm_safe(
    amm_data: &web::Data<Mutex<MultiTickAMM>>
) -> Result<std::sync::MutexGuard<MultiTickAMM>, String> {
    match amm_data.lock() {
        Ok(guard) => Ok(guard),
        Err(poisoned) => {
            eprintln!("Mutex was poisoned, recovering...");
            Ok(poisoned.into_inner())
        }
    }
}

pub async fn run(
    addr: &str,
    port: u16,
    token_names: Vec<String>,
    initial_reserves: Vec<f64>,
    initial_plane: f64
) -> std::io::Result<()> {
    // Initialize or load existing state
    let mut amm = MultiTickAMM::load_state(token_names.clone());

    // If empty, add a tick with specified configuration
    if amm.ticks.is_empty() {
        let reserves = if initial_reserves.len() == token_names.len() {
            initial_reserves
        } else {
            vec![1000.0; token_names.len()] // fallback
        };

        amm.add_tick(initial_plane, reserves.clone());
        amm.save_state();
        println!("Initialized with tick: plane={}, reserves={:?}", initial_plane, reserves);
    }

    let amm_data = web::Data::new(Mutex::new(amm));

    println!("Server running at http://{}:{}", addr, port);
    println!("Tokens: {:?}", token_names);

    HttpServer::new(move || {
        let cors = Cors::default().allow_any_origin().allow_any_method().allow_any_header();

        App::new()
            .app_data(amm_data.clone())
            .wrap(cors)
            .wrap(Logger::default())
            .service(get_state)
            .service(post_trade)
            .service(post_tick)
            .service(get_prices)
            .service(reset_state)
            .service(set_reserves)
            .service(add_liquidity)
            .service(remove_liquidity)
            .service(get_price_single)
            .service(reconfigure_amm)
            .service(
                fs::Files::new("/", "../web/dist").index_file("index.html").show_files_listing()
            )
    })
        .bind((addr, port))?
        .run().await
}

#[derive(Serialize)]
struct StateResponse {
    ticks: Vec<TickInfo>,
    token_names: Vec<String>,
    global_reserves: Vec<f64>,
    tick_count: usize,
}

#[derive(Serialize)]
struct TickInfo {
    index: usize,
    plane_constant: f64,
    reserves: Vec<f64>,
    radius: f64,
    is_interior: bool,
    is_boundary: bool,
    liquidity: f64,
}

#[get("/api/state")]
async fn get_state(amm: web::Data<Mutex<MultiTickAMM>>) -> impl Responder {
    let state = match get_amm_safe(&amm) {
        Ok(guard) => guard,
        Err(e) => {
            return HttpResponse::InternalServerError().json(serde_json::json!({"error": e}));
        }
    };

    let tick_infos: Vec<TickInfo> = state.ticks
        .iter()
        .enumerate()
        .map(|(i, tick)| {
            TickInfo {
                index: i,
                plane_constant: tick.plane_constant,
                reserves: tick.sphere_amm.reserves.clone(),
                radius: tick.sphere_amm.radius,
                is_interior: tick.is_interior(),
                is_boundary: tick.is_boundary(),
                liquidity: tick.liquidity(),
            }
        })
        .collect();

    let response = StateResponse {
        ticks: tick_infos,
        token_names: state.token_names.clone(),
        global_reserves: state.global_reserves.clone(),
        tick_count: state.ticks.len(),
    };

    HttpResponse::Ok().json(response)
}

#[derive(Deserialize)]
struct ReconfigureReq {
    token_names: Vec<String>,
    initial_reserves: Vec<f64>,
    initial_plane: f64,
}

#[post("/api/reconfigure")]
async fn reconfigure_amm(
    amm: web::Data<Mutex<MultiTickAMM>>,
    json: web::Json<ReconfigureReq>
) -> impl Responder {
    let mut amm_guard = match get_amm_safe(&amm) {
        Ok(guard) => guard,
        Err(e) => {
            return HttpResponse::InternalServerError().json(serde_json::json!({"error": e}));
        }
    };

    // Validation
    if json.initial_reserves.len() != json.token_names.len() {
        return HttpResponse::BadRequest().json(
            serde_json::json!({
            "success": false,
            "message": "Token names and reserves length mismatch"
        })
        );
    }

    if json.token_names.is_empty() {
        return HttpResponse::BadRequest().json(
            serde_json::json!({
            "success": false,
            "message": "At least one token is required"
        })
        );
    }

    if json.initial_reserves.iter().any(|&r| r < 0.0) {
        return HttpResponse::BadRequest().json(
            serde_json::json!({
            "success": false,
            "message": "All reserves must be non-negative"
        })
        );
    }

    if json.initial_plane <= 0.0 {
        return HttpResponse::BadRequest().json(
            serde_json::json!({
            "success": false,
            "message": "Plane constant must be positive"
        })
        );
    }

    // Create completely new AMM with new configuration
    *amm_guard = MultiTickAMM::new(json.token_names.clone());

    // Add initial tick with specified configuration
    amm_guard.add_tick(json.initial_plane, json.initial_reserves.clone());
    amm_guard.save_state();

    HttpResponse::Ok().json(
        serde_json::json!({
        "success": true,
        "message": format!("AMM reconfigured with tokens: {:?}", json.token_names)
    })
    )
}

#[derive(Deserialize)]
struct ResetConfig {
    reserves: Vec<f64>,
    plane: f64,
}

#[derive(Deserialize)]
struct TradeReq {
    from: String,
    to: String,
    amount: f64,
}

#[derive(Serialize)]
struct TradeResponse {
    output: f64,
    success: bool,
    message: String,
}

#[post("/api/trade")]
async fn post_trade(
    amm: web::Data<Mutex<MultiTickAMM>>,
    json: web::Json<TradeReq>
) -> impl Responder {
    let mut amm_guard = match get_amm_safe(&amm) {
        Ok(guard) => guard,
        Err(e) => {
            return HttpResponse::InternalServerError().json(serde_json::json!({"error": e}));
        }
    };

    match amm_guard.route_trade(&json.from, &json.to, json.amount) {
        Ok(output) => {
            amm_guard.save_state();
            let response = TradeResponse {
                output,
                success: true,
                message: format!(
                    "Swapped {} {} for {} {}",
                    json.amount,
                    json.from,
                    output,
                    json.to
                ),
            };
            HttpResponse::Ok().json(response)
        }
        Err(e) => {
            let response = TradeResponse {
                output: 0.0,
                success: false,
                message: format!("Trade failed: {}", e),
            };
            HttpResponse::BadRequest().json(response)
        }
    }
}

#[derive(Deserialize)]
struct TickReq {
    plane: f64,
    reserves: Vec<f64>,
}

#[post("/api/tick")]
async fn post_tick(
    amm: web::Data<Mutex<MultiTickAMM>>,
    json: web::Json<TickReq>
) -> impl Responder {
    let mut amm_guard = match get_amm_safe(&amm) {
        Ok(guard) => guard,
        Err(e) => {
            return HttpResponse::InternalServerError().json(serde_json::json!({"error": e}));
        }
    };

    if json.reserves.len() != amm_guard.token_names.len() {
        return HttpResponse::BadRequest().json(
            serde_json::json!({
            "success": false,
            "message": "Reserve length mismatch"
        })
        );
    }

    amm_guard.add_tick(json.plane, json.reserves.clone());
    amm_guard.save_state();

    HttpResponse::Ok().json(
        serde_json::json!({
        "success": true,
        "message": format!("Added tick with plane constant {}", json.plane)
    })
    )
}

#[derive(Serialize)]
struct PriceInfo {
    from: String,
    to: String,
    price: f64,
}

#[get("/api/prices")]
async fn get_prices(amm: web::Data<Mutex<MultiTickAMM>>) -> impl Responder {
    let state = match get_amm_safe(&amm) {
        Ok(guard) => guard,
        Err(e) => {
            return HttpResponse::InternalServerError().json(serde_json::json!({"error": e}));
        }
    };

    let mut prices = Vec::new();

    for i in 0..state.token_names.len() {
        for j in 0..state.token_names.len() {
            if i != j {
                if
                    let Ok(price) = state.get_aggregated_price(
                        &state.token_names[i],
                        &state.token_names[j]
                    )
                {
                    prices.push(PriceInfo {
                        from: state.token_names[i].clone(),
                        to: state.token_names[j].clone(),
                        price,
                    });
                }
            }
        }
    }

    HttpResponse::Ok().json(prices)
}

#[derive(Deserialize)]
struct SetReservesReq {
    tick_index: usize,
    reserves: Vec<f64>,
}

#[derive(Deserialize)]
struct AddLiquidityReq {
    tick_index: usize,
    lp_id: String,
    amounts: Vec<f64>,
}

#[derive(Deserialize)]
struct RemoveLiquidityReq {
    tick_index: usize,
    lp_id: String,
    percentage: f64,
}

#[post("/api/set-reserves")]
async fn set_reserves(
    amm: web::Data<Mutex<MultiTickAMM>>,
    json: web::Json<SetReservesReq>
) -> impl Responder {
    let mut amm_guard = match get_amm_safe(&amm) {
        Ok(guard) => guard,
        Err(e) => {
            return HttpResponse::InternalServerError().json(serde_json::json!({"error": e}));
        }
    };

    if json.tick_index >= amm_guard.ticks.len() {
        return HttpResponse::BadRequest().json(
            serde_json::json!({
            "success": false,
            "message": "Invalid tick index"
        })
        );
    }

    if json.reserves.len() != amm_guard.token_names.len() {
        return HttpResponse::BadRequest().json(
            serde_json::json!({
            "success": false,
            "message": "Reserve length mismatch"
        })
        );
    }

    // Validation
    if json.reserves.iter().any(|&r| r < 0.0) {
        return HttpResponse::BadRequest().json(
            serde_json::json!({
            "success": false,
            "message": "All reserves must be non-negative"
        })
        );
    }

    // Directly set reserves and recalculate radius
    let tick = &mut amm_guard.ticks[json.tick_index];
    tick.sphere_amm.reserves = json.reserves.clone();

    // Recalculate radius to maintain sphere constraint
    let n = tick.sphere_amm.reserves.len() as f64;
    let sum_x: f64 = tick.sphere_amm.reserves.iter().sum();
    let sum_x2: f64 = tick.sphere_amm.reserves
        .iter()
        .map(|x| x * x)
        .sum();
    let a = n - 1.0;

    tick.sphere_amm.radius = if a.abs() < 1e-12 {
        sum_x
    } else {
        let b = -2.0 * sum_x;
        let c = sum_x2;
        let disc = (b * b - 4.0 * a * c).max(0.0);
        let r1 = (-b + disc.sqrt()) / (2.0 * a);
        if r1 > 0.0 {
            r1
        } else {
            (-b - disc.sqrt()) / (2.0 * a)
        }
    };

    amm_guard.save_state();

    HttpResponse::Ok().json(
        serde_json::json!({
        "success": true,
        "message": format!("Set reserves for tick {}", json.tick_index)
    })
    )
}

#[post("/api/add-liquidity")]
async fn add_liquidity(
    amm: web::Data<Mutex<MultiTickAMM>>,
    json: web::Json<AddLiquidityReq>
) -> impl Responder {
    let mut amm_guard = match get_amm_safe(&amm) {
        Ok(guard) => guard,
        Err(e) => {
            return HttpResponse::InternalServerError().json(serde_json::json!({"error": e}));
        }
    };

    if json.tick_index >= amm_guard.ticks.len() {
        return HttpResponse::BadRequest().json(
            serde_json::json!({
            "success": false,
            "message": "Invalid tick index"
        })
        );
    }

    let tick = &mut amm_guard.ticks[json.tick_index];
    match tick.add_liquidity(&json.lp_id, &json.amounts) {
        Ok(_) => {
            amm_guard.save_state();
            HttpResponse::Ok().json(
                serde_json::json!({
                "success": true,
                "message": format!("Added liquidity for LP {}", json.lp_id)
            })
            )
        }
        Err(e) =>
            HttpResponse::BadRequest().json(
                serde_json::json!({
            "success": false,
            "message": e
        })
            ),
    }
}

#[post("/api/remove-liquidity")]
async fn remove_liquidity(
    amm: web::Data<Mutex<MultiTickAMM>>,
    json: web::Json<RemoveLiquidityReq>
) -> impl Responder {
    let mut amm_guard = match get_amm_safe(&amm) {
        Ok(guard) => guard,
        Err(e) => {
            return HttpResponse::InternalServerError().json(serde_json::json!({"error": e}));
        }
    };

    if json.tick_index >= amm_guard.ticks.len() {
        return HttpResponse::BadRequest().json(
            serde_json::json!({
            "success": false,
            "message": "Invalid tick index"
        })
        );
    }

    let tick = &mut amm_guard.ticks[json.tick_index];
    match tick.withdraw_liquidity(&json.lp_id, json.percentage) {
        Ok(withdrawn) => {
            amm_guard.save_state();
            HttpResponse::Ok().json(
                serde_json::json!({
                "success": true,
                "message": format!("Removed liquidity for LP {}", json.lp_id),
                "withdrawn": withdrawn
            })
            )
        }
        Err(e) =>
            HttpResponse::BadRequest().json(
                serde_json::json!({
            "success": false,
            "message": e
        })
            ),
    }
}

#[get("/api/price")]
async fn get_price_single(
    amm: web::Data<Mutex<MultiTickAMM>>,
    query: web::Query<HashMap<String, String>>
) -> impl Responder {
    let state = match get_amm_safe(&amm) {
        Ok(guard) => guard,
        Err(e) => {
            return HttpResponse::InternalServerError().json(serde_json::json!({"error": e}));
        }
    };

    let from = match query.get("from") {
        Some(f) => f,
        None => {
            return HttpResponse::BadRequest().json(
                serde_json::json!({"error": "Missing from parameter"})
            );
        }
    };

    let to = match query.get("to") {
        Some(t) => t,
        None => {
            return HttpResponse::BadRequest().json(
                serde_json::json!({"error": "Missing to parameter"})
            );
        }
    };

    match state.get_aggregated_price(from, to) {
        Ok(price) =>
            HttpResponse::Ok().json(
                serde_json::json!({
            "price": price,
            "from": from,
            "to": to
        })
            ),
        Err(e) => HttpResponse::BadRequest().json(serde_json::json!({"error": e})),
    }
}

#[post("/api/reset")]
async fn reset_state(amm: web::Data<Mutex<MultiTickAMM>>) -> impl Responder {
    let mut amm_guard = match get_amm_safe(&amm) {
        Ok(guard) => guard,
        Err(e) => {
            return HttpResponse::InternalServerError().json(serde_json::json!({"error": e}));
        }
    };

    let token_names = amm_guard.token_names.clone();

    // Reset to fresh state
    *amm_guard = MultiTickAMM::new(token_names.clone());

    // Add default tick
    let default_reserves = vec![1000.0; token_names.len()];
    amm_guard.add_tick(600.0, default_reserves);
    amm_guard.save_state();

    HttpResponse::Ok().json(
        serde_json::json!({
        "success": true,
        "message": "AMM state reset with default tick"
    })
    )
}
