mod sphere;
mod ticks;
mod server;

use clap::{ Parser, Subcommand };
use sphere::SphereAMM;

#[derive(Parser)]
#[command(name = "orbital")]
#[command(about = "A simple CLI simulator for Paradigm's Orbital AMM")]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Initialize a new Orbital pool with specified reserves
    Init {
        /// Initial reserves for each token (format: "TOKEN:AMOUNT")
        #[arg(value_delimiter = ',')]
        reserves: Vec<String>,
    },
    /// Execute a swap between tokens
    Swap {
        /// Token to swap from
        from: String,
        /// Token to swap to
        to: String,
        /// Amount to swap
        amount: f64,
    },
    /// Show current pool state
    State,
    /// Show price of a token in terms of another
    Price {
        /// Base token
        base: String,
        /// Quote token
        quote: String,
    },
    /// Run web server
    Server {
        /// Port to run on
        #[arg(short, long, default_value = "8080")]
        port: u16,
        /// Address to bind to
        #[arg(short, long, default_value = "127.0.0.1")]
        addr: String,
        /// Tokens to use (format: "TOKEN1,TOKEN2,TOKEN3")
        #[arg(short, long, default_value = "USDC,USDT,DAI")]
        tokens: String,
        /// Initial reserves for default tick (format: "1000,1000,1000")
        #[arg(short, long, default_value = "1000,1000,1000")]
        reserves: String,
        /// Initial plane constant for default tick
        #[arg(long, default_value = "600")]
        plane: f64,
    },
}

#[tokio::main]
async fn main() {
    let cli = Cli::parse();

    match &cli.command {
        Commands::Init { reserves } => {
            let mut token_names = Vec::new();
            let mut amounts = Vec::new();
            for reserve in reserves {
                let parts: Vec<&str> = reserve.split(':').collect();
                if parts.len() == 2 {
                    token_names.push(parts[0].to_string());
                    let amount: f64 = parts[1].parse().unwrap_or(0.0);
                    amounts.push(amount);
                }
            }
            let pool = SphereAMM::new(token_names, amounts);
            pool.save_state();
            println!("Pool initialised with {} tokens", pool.token_names.len());
        }
        Commands::Swap { from, to, amount } => {
            let mut pool = SphereAMM::load_state();
            match pool.swap(from, to, *amount) {
                Ok(output_amount) => {
                    println!("Swapped {} {} for {} {}", amount, from, output_amount, to);
                    pool.save_state();
                }
                Err(e) => println!("Error: {}", e),
            }
        }
        Commands::State => {
            let pool = SphereAMM::load_state();
            pool.print_state();
        }
        Commands::Price { base, quote } => {
            let pool = SphereAMM::load_state();
            match pool.get_spot_price(base, quote) {
                Ok(price) => println!("Spot price of {} in {}: {}", quote, base, price),
                Err(e) => println!("Error: {}", e),
            }
        }
        Commands::Server { port, addr, tokens, reserves, plane } => {
            println!("Starting Orbital server on {}:{}", addr, port);

            // Parse tokens
            let token_names: Vec<String> = tokens
                .split(',')
                .map(|s| s.trim().to_string())
                .collect();

            // Parse initial reserves
            let initial_reserves: Vec<f64> = reserves
                .split(',')
                .map(|s| s.trim().parse().unwrap_or(1000.0))
                .collect();

            println!("Tokens: {:?}", token_names);
            println!("Initial reserves: {:?}", initial_reserves);
            println!("Initial plane constant: {}", plane);

            if let Err(e) = server::run(addr, *port, token_names, initial_reserves, *plane).await {
                eprintln!("Server error: {}", e);
            }
        }
    }
}
