// @ts-nocheck
import { useEffect, useState } from 'react';
import { Line, Bar } from 'react-chartjs-2';
import {
    Chart as ChartJS,
    CategoryScale,
    LinearScale,
    PointElement,
    LineElement,
    BarElement,
    Title,
    Tooltip,
    Legend,
} from 'chart.js';

ChartJS.register(CategoryScale, LinearScale, PointElement, LineElement, BarElement, Title, Tooltip, Legend);

interface TickInfo {
    index: number;
    plane_constant: number;
    reserves: number[];
    radius: number;
    is_interior: boolean;
    is_boundary: boolean;
    liquidity: number;
}

interface State {
    ticks: TickInfo[];
    token_names: string[];
    global_reserves: number[];
    tick_count: number;
}

const API_PREFIX = window.location.hostname === 'localhost' ? 'http://localhost:8080/api' : '/api';

const colors = ['#3b82f6', '#ef4444', '#10b981', '#f59e0b', '#8b5cf6'];

export default function App() {
    const [state, setState] = useState<State | null>(null);
    const [plane, setPlane] = useState('');
    const [tickRes, setTickRes] = useState('');
    const [tradeFrom, setTradeFrom] = useState('');
    const [tradeTo, setTradeTo] = useState('');
    const [tradeAmt, setTradeAmt] = useState('');
    const [status, setStatus] = useState('');
    const [selectedTick, setSelectedTick] = useState(0);
    const [newReserves, setNewReserves] = useState('');
    const [lpId, setLpId] = useState('');
    const [lpAmounts, setLpAmounts] = useState('');
    const [resetReserves, setResetReserves] = useState('');
    const [resetPlane, setResetPlane] = useState('600');
    const [configTokens, setConfigTokens] = useState('');
    const [configReserves, setConfigReserves] = useState('');
    const [configPlane, setConfigPlane] = useState('600');
    const [showConfig, setShowConfig] = useState(false);

    async function fetchState() {
        try {
            const res = await fetch(`${API_PREFIX}/state`);
            if (res.ok) {
                const data = await res.json();
                setState(data);
                if (data.token_names.length > 0 && !tradeFrom) {
                    setTradeFrom(data.token_names[0]);
                    setTradeTo(data.token_names[1] || data.token_names[0]);
                }
            }
        } catch (e) {
            setStatus('Failed to connect to server');
        }
    }

    useEffect(() => {
        fetchState();
        const id = setInterval(fetchState, 3000);
        return () => clearInterval(id);
    }, []);

    const showStatus = (msg: string, type: 'success' | 'error' = 'success') => {
        setStatus(msg);
        setTimeout(() => setStatus(''), 3000);
    };

    const addTick = async () => {
        if (!plane || !tickRes) return;
        const resv = tickRes.split(',').map(Number);
        try {
            const res = await fetch(`${API_PREFIX}/tick`, {
                method: 'POST',
                headers: { 'Content-Type': 'application/json' },
                body: JSON.stringify({ plane: parseFloat(plane), reserves: resv }),
            });
            const result = await res.json();
            if (result.success) {
                setPlane('');
                setTickRes('');
                showStatus('Tick added successfully');
                fetchState();
            } else {
                showStatus(result.message, 'error');
            }
        } catch (e) {
            showStatus('Failed to add tick', 'error');
        }
    };

    const executeTrade = async () => {
        if (!tradeFrom || !tradeTo || !tradeAmt) return;
        try {
            const res = await fetch(`${API_PREFIX}/trade`, {
                method: 'POST',
                headers: { 'Content-Type': 'application/json' },
                body: JSON.stringify({ from: tradeFrom, to: tradeTo, amount: parseFloat(tradeAmt) }),
            });
            const result = await res.json();
            if (result.success) {
                setTradeAmt('');
                showStatus(`Trade successful: ${result.output.toFixed(4)} ${tradeTo} received`);
                fetchState();
            } else {
                showStatus(result.message, 'error');
            }
        } catch (e) {
            showStatus('Trade failed', 'error');
        }
    };

    const setReserves = async () => {
        if (!newReserves) return;
        const reserves = newReserves.split(',').map(r => parseFloat(r.trim()));
        try {
            const res = await fetch(`${API_PREFIX}/set-reserves`, {
                method: 'POST',
                headers: { 'Content-Type': 'application/json' },
                body: JSON.stringify({ tick_index: selectedTick, reserves }),
            });
            const result = await res.json();
            if (result.success) {
                setNewReserves('');
                showStatus('Reserves updated');
                fetchState();
            } else {
                showStatus(result.message, 'error');
            }
        } catch (e) {
            showStatus('Failed to set reserves', 'error');
        }
    };

    const addLiquidity = async () => {
        if (!lpId || !lpAmounts) return;
        const amounts = lpAmounts.split(',').map(a => parseFloat(a.trim()));
        try {
            const res = await fetch(`${API_PREFIX}/add-liquidity`, {
                method: 'POST',
                headers: { 'Content-Type': 'application/json' },
                body: JSON.stringify({ tick_index: selectedTick, lp_id: lpId, amounts }),
            });
            const result = await res.json();
            if (result.success) {
                setLpId('');
                setLpAmounts('');
                showStatus('Liquidity added');
                fetchState();
            } else {
                showStatus(result.message, 'error');
            }
        } catch (e) {
            showStatus('Failed to add liquidity', 'error');
        }
    };

    const resetAMM = async () => {
        if (!confirm('Reset all ticks?')) return;
        try {
            const resetConfig = resetReserves ? {
                reserves: resetReserves.split(',').map(r => parseFloat(r.trim())),
                plane: parseFloat(resetPlane)
            } : null;

            await fetch(`${API_PREFIX}/reset`, {
                method: 'POST',
                headers: { 'Content-Type': 'application/json' },
                body: JSON.stringify(resetConfig)
            });
            showStatus('AMM reset');
            fetchState();
        } catch (e) {
            showStatus('Reset failed', 'error');
        }
    };

    const reconfigureAMM = async () => {
        if (!configTokens || !configReserves) {
            showStatus('Please fill in tokens and reserves', 'error');
            return;
        }

        if (!confirm('This will completely reconfigure the AMM and delete all existing ticks. Continue?')) {
            return;
        }

        try {
            const token_names = configTokens.split(',').map(t => t.trim());
            const initial_reserves = configReserves.split(',').map(r => parseFloat(r.trim()));

            if (token_names.length !== initial_reserves.length) {
                showStatus('Number of tokens and reserves must match', 'error');
                return;
            }

            const res = await fetch(`${API_PREFIX}/reconfigure`, {
                method: 'POST',
                headers: { 'Content-Type': 'application/json' },
                body: JSON.stringify({
                    token_names,
                    initial_reserves,
                    initial_plane: parseFloat(configPlane)
                })
            });

            const result = await res.json();
            if (result.success) {
                showStatus('AMM reconfigured successfully');
                setConfigTokens('');
                setConfigReserves('');
                setShowConfig(false);
                fetchState();
            } else {
                showStatus(result.message, 'error');
            }
        } catch (e) {
            showStatus('Reconfiguration failed', 'error');
        }
    };

    if (!state) {
        return (
            <div style={{ padding: 40, textAlign: 'center', color: '#666' }}>
                <h2>Loading Orbital AMM...</h2>
                <p>Make sure the server is running on port 8080</p>
            </div>
        );
    }

    // Chart data
    const reservesData = {
        labels: state.ticks.map((_, i) => `Tick ${i}`),
        datasets: state.token_names.map((token, i) => ({
            label: token,
            data: state.ticks.map(t => t.reserves[i]),
            backgroundColor: colors[i] + '40',
            borderColor: colors[i],
            borderWidth: 2,
        })),
    };

    const liquidityData = {
        labels: state.ticks.map((_, i) => `Tick ${i}`),
        datasets: [{
            label: 'Total Liquidity',
            data: state.ticks.map(t => t.liquidity),
            backgroundColor: '#3b82f680',
            borderColor: '#3b82f6',
            borderWidth: 2,
        }],
    };

    const chartOptions = {
        responsive: true,
        maintainAspectRatio: false,
        plugins: {
            legend: { position: 'top' as const },
        },
        scales: {
            y: { beginAtZero: true },
        },
    };

    return (
        <div style={{
            fontFamily: 'system-ui, sans-serif',
            maxWidth: '1200px',
            margin: '0 auto',
            padding: '20px',
            backgroundColor: '#f8fafc'
        }}>
            <div style={{
                backgroundColor: 'white',
                padding: '24px',
                borderRadius: '12px',
                marginBottom: '20px',
                boxShadow: '0 1px 3px rgba(0,0,0,0.1)'
            }}>
                <h1 style={{ margin: '0 0 8px 0', color: '#1e293b', fontSize: '28px' }}>
                    🌌 Orbital AMM Sandbox
                </h1>
                <p style={{ margin: 0, color: '#64748b' }}>
                    Interactive multi-tick stablecoin AMM with sphere constraints
                </p>
            </div>

            {status && (
                <div style={{
                    padding: '12px 16px',
                    borderRadius: '8px',
                    marginBottom: '20px',
                    backgroundColor: status.includes('failed') || status.includes('Failed') ? '#fee2e2' : '#dcfce7',
                    color: status.includes('failed') || status.includes('Failed') ? '#dc2626' : '#166534',
                    border: `1px solid ${status.includes('failed') || status.includes('Failed') ? '#fecaca' : '#bbf7d0'}`
                }}>
                    {status}
                </div>
            )}

            <div style={{ display: 'grid', gridTemplateColumns: '1fr 1fr', gap: '20px', marginBottom: '20px' }}>
                <div style={{ backgroundColor: 'white', padding: '20px', borderRadius: '12px', boxShadow: '0 1px 3px rgba(0,0,0,0.1)' }}>
                    <h2 style={{ margin: '0 0 16px 0', color: '#1e293b' }}>📊 Reserves by Tick</h2>
                    <div style={{ height: '300px' }}>
                        <Bar data={reservesData} options={chartOptions} />
                    </div>
                </div>

                <div style={{ backgroundColor: 'white', padding: '20px', borderRadius: '12px', boxShadow: '0 1px 3px rgba(0,0,0,0.1)' }}>
                    <h2 style={{ margin: '0 0 16px 0', color: '#1e293b' }}>💧 Total Liquidity</h2>
                    <div style={{ height: '300px' }}>
                        <Bar data={liquidityData} options={chartOptions} />
                    </div>
                </div>
            </div>

            <div style={{ backgroundColor: 'white', padding: '20px', borderRadius: '12px', marginBottom: '20px', boxShadow: '0 1px 3px rgba(0,0,0,0.1)' }}>
                <h2 style={{ margin: '0 0 16px 0', color: '#1e293b' }}>🎯 Tick Details</h2>
                <div style={{ overflowX: 'auto' }}>
                    <table style={{ width: '100%', borderCollapse: 'collapse' }}>
                        <thead>
                            <tr style={{ backgroundColor: '#f8fafc' }}>
                                <th style={{ padding: '8px 12px', textAlign: 'left', borderBottom: '2px solid #e2e8f0' }}>#</th>
                                <th style={{ padding: '8px 12px', textAlign: 'left', borderBottom: '2px solid #e2e8f0' }}>Plane (c)</th>
                                <th style={{ padding: '8px 12px', textAlign: 'left', borderBottom: '2px solid #e2e8f0' }}>Status</th>
                                <th style={{ padding: '8px 12px', textAlign: 'left', borderBottom: '2px solid #e2e8f0' }}>Reserves</th>
                                <th style={{ padding: '8px 12px', textAlign: 'left', borderBottom: '2px solid #e2e8f0' }}>Radius</th>
                                <th style={{ padding: '8px 12px', textAlign: 'left', borderBottom: '2px solid #e2e8f0' }}>Liquidity</th>
                            </tr>
                        </thead>
                        <tbody>
                            {state.ticks.map((tick, i) => (
                                <tr key={i} style={{ backgroundColor: i % 2 === 0 ? 'white' : '#f8fafc' }}>
                                    <td style={{ padding: '8px 12px', borderBottom: '1px solid #e2e8f0' }}>{i}</td>
                                    <td style={{ padding: '8px 12px', borderBottom: '1px solid #e2e8f0' }}>{tick.plane_constant.toFixed(2)}</td>
                                    <td style={{ padding: '8px 12px', borderBottom: '1px solid #e2e8f0' }}>
                                        <span style={{
                                            padding: '2px 8px',
                                            borderRadius: '12px',
                                            fontSize: '12px',
                                            fontWeight: '500',
                                            backgroundColor: tick.is_interior ? '#dcfce7' : tick.is_boundary ? '#fef3c7' : '#f1f5f9',
                                            color: tick.is_interior ? '#166534' : tick.is_boundary ? '#92400e' : '#475569'
                                        }}>
                                            {tick.is_interior ? 'Interior' : tick.is_boundary ? 'Boundary' : 'Other'}
                                        </span>
                                    </td>
                                    <td style={{ padding: '8px 12px', borderBottom: '1px solid #e2e8f0', fontFamily: 'monospace' }}>
                                        [{tick.reserves.map(r => r.toFixed(1)).join(', ')}]
                                    </td>
                                    <td style={{ padding: '8px 12px', borderBottom: '1px solid #e2e8f0' }}>{tick.radius.toFixed(2)}</td>
                                    <td style={{ padding: '8px 12px', borderBottom: '1px solid #e2e8f0' }}>{tick.liquidity.toFixed(2)}</td>
                                </tr>
                            ))}
                        </tbody>
                    </table>
                </div>
            </div>

            <div style={{ display: 'grid', gridTemplateColumns: '1fr 1fr', gap: '20px' }}>
                <div style={{ backgroundColor: 'white', padding: '20px', borderRadius: '12px', boxShadow: '0 1px 3px rgba(0,0,0,0.1)' }}>
                    <h2 style={{ margin: '0 0 16px 0', color: '#1e293b' }}>⚡ Execute Trade</h2>
                    <div style={{ display: 'grid', gap: '12px' }}>
                        <div style={{ display: 'grid', gridTemplateColumns: '1fr 1fr', gap: '8px' }}>
                            <select
                                value={tradeFrom}
                                onChange={e => setTradeFrom(e.target.value)}
                                style={{ padding: '8px', border: '1px solid #d1d5db', borderRadius: '6px' }}
                            >
                                {state.token_names.map(token => (
                                    <option key={token} value={token}>{token}</option>
                                ))}
                            </select>
                            <select
                                value={tradeTo}
                                onChange={e => setTradeTo(e.target.value)}
                                style={{ padding: '8px', border: '1px solid #d1d5db', borderRadius: '6px' }}
                            >
                                {state.token_names.map(token => (
                                    <option key={token} value={token}>{token}</option>
                                ))}
                            </select>
                        </div>
                        <input
                            type="number"
                            placeholder="Amount to trade"
                            value={tradeAmt}
                            onChange={e => setTradeAmt(e.target.value)}
                            style={{ padding: '8px', border: '1px solid #d1d5db', borderRadius: '6px' }}
                        />
                        <button
                            onClick={executeTrade}
                            style={{
                                padding: '10px',
                                backgroundColor: '#3b82f6',
                                color: 'white',
                                border: 'none',
                                borderRadius: '6px',
                                fontWeight: '500',
                                cursor: 'pointer'
                            }}
                        >
                            Execute Trade
                        </button>
                    </div>

                    <h3 style={{ margin: '24px 0 12px 0', color: '#1e293b' }}>➕ Add New Tick</h3>
                    <div style={{ display: 'grid', gap: '8px' }}>
                        <input
                            type="number"
                            placeholder="Plane constant (e.g., 500)"
                            value={plane}
                            onChange={e => setPlane(e.target.value)}
                            style={{ padding: '8px', border: '1px solid #d1d5db', borderRadius: '6px' }}
                        />
                        <input
                            placeholder="Initial reserves (e.g., 1000,1000,1000)"
                            value={tickRes}
                            onChange={e => setTickRes(e.target.value)}
                            style={{ padding: '8px', border: '1px solid #d1d5db', borderRadius: '6px' }}
                        />
                        <button
                            onClick={addTick}
                            style={{
                                padding: '8px',
                                backgroundColor: '#10b981',
                                color: 'white',
                                border: 'none',
                                borderRadius: '6px',
                                fontWeight: '500',
                                cursor: 'pointer'
                            }}
                        >
                            Add Tick
                        </button>
                    </div>
                </div>

                <div style={{ backgroundColor: 'white', padding: '20px', borderRadius: '12px', boxShadow: '0 1px 3px rgba(0,0,0,0.1)' }}>
                    <h2 style={{ margin: '0 0 16px 0', color: '#1e293b' }}>🔧 Advanced Controls</h2>

                    <h3 style={{ margin: '0 0 8px 0', fontSize: '16px', color: '#374151' }}>Set Reserves</h3>
                    <div style={{ display: 'grid', gap: '8px', marginBottom: '16px' }}>
                        <select
                            value={selectedTick}
                            onChange={e => setSelectedTick(parseInt(e.target.value))}
                            style={{ padding: '8px', border: '1px solid #d1d5db', borderRadius: '6px' }}
                        >
                            {state.ticks.map((_, i) => (
                                <option key={i} value={i}>Tick {i} (plane={state.ticks[i].plane_constant.toFixed(2)})</option>
                            ))}
                        </select>
                        <input
                            placeholder="New reserves (comma separated)"
                            value={newReserves}
                            onChange={e => setNewReserves(e.target.value)}
                            style={{ padding: '8px', border: '1px solid #d1d5db', borderRadius: '6px' }}
                        />
                        <button
                            onClick={setReserves}
                            style={{
                                padding: '8px',
                                backgroundColor: '#f59e0b',
                                color: 'white',
                                border: 'none',
                                borderRadius: '6px',
                                fontWeight: '500',
                                cursor: 'pointer'
                            }}
                        >
                            Set Reserves
                        </button>
                    </div>

                    <h3 style={{ margin: '0 0 8px 0', fontSize: '16px', color: '#374151' }}>Add Liquidity</h3>
                    <div style={{ display: 'grid', gap: '8px', marginBottom: '16px' }}>
                        <input
                            placeholder="LP ID"
                            value={lpId}
                            onChange={e => setLpId(e.target.value)}
                            style={{ padding: '8px', border: '1px solid #d1d5db', borderRadius: '6px' }}
                        />
                        <input
                            placeholder="Amounts (comma separated)"
                            value={lpAmounts}
                            onChange={e => setLpAmounts(e.target.value)}
                            style={{ padding: '8px', border: '1px solid #d1d5db', borderRadius: '6px' }}
                        />
                        <button
                            onClick={addLiquidity}
                            style={{
                                padding: '8px',
                                backgroundColor: '#8b5cf6',
                                color: 'white',
                                border: 'none',
                                borderRadius: '6px',
                                fontWeight: '500',
                                cursor: 'pointer'
                            }}
                        >
                            Add Liquidity
                        </button>
                    </div>

                    <h3 style={{ margin: '0 0 8px 0', fontSize: '16px', color: '#374151' }}>Reset AMM</h3>
                    <div style={{ display: 'grid', gap: '8px', marginBottom: '16px' }}>
                        <input
                            placeholder="Reset reserves (optional, e.g., 1500,1500,1500)"
                            value={resetReserves}
                            onChange={e => setResetReserves(e.target.value)}
                            style={{ padding: '8px', border: '1px solid #d1d5db', borderRadius: '6px' }}
                        />
                        <input
                            type="number"
                            placeholder="Reset plane constant"
                            value={resetPlane}
                            onChange={e => setResetPlane(e.target.value)}
                            style={{ padding: '8px', border: '1px solid #d1d5db', borderRadius: '6px' }}
                        />
                        <button
                            onClick={resetAMM}
                            style={{
                                padding: '8px 16px',
                                backgroundColor: '#ef4444',
                                color: 'white',
                                border: 'none',
                                borderRadius: '6px',
                                fontWeight: '500',
                                cursor: 'pointer'
                            }}
                        >
                            🗑️ Reset AMM
                        </button>
                    </div>
                </div>
            </div>

            <div style={{
                backgroundColor: 'white',
                padding: '20px',
                borderRadius: '12px',
                marginTop: '20px',
                boxShadow: '0 1px 3px rgba(0,0,0,0.1)'
            }}>
                <div style={{ display: 'flex', justifyContent: 'space-between', alignItems: 'center', marginBottom: '16px' }}>
                    <h2 style={{ margin: 0, color: '#1e293b' }}>⚙️ AMM Configuration</h2>
                    <button
                        onClick={() => setShowConfig(!showConfig)}
                        style={{
                            padding: '6px 12px',
                            backgroundColor: showConfig ? '#ef4444' : '#3b82f6',
                            color: 'white',
                            border: 'none',
                            borderRadius: '6px',
                            fontSize: '12px',
                            cursor: 'pointer'
                        }}
                    >
                        {showConfig ? 'Cancel' : 'Reconfigure'}
                    </button>
                </div>

                {showConfig ? (
                    <div style={{
                        padding: '16px',
                        backgroundColor: '#fef3c7',
                        borderRadius: '8px',
                        border: '1px solid #f59e0b',
                        marginBottom: '16px'
                    }}>
                        <h3 style={{ margin: '0 0 12px 0', color: '#92400e' }}>⚠️ Complete Reconfiguration</h3>
                        <p style={{ margin: '0 0 16px 0', fontSize: '14px', color: '#92400e' }}>
                            This will delete all existing ticks and create a new AMM with different tokens.
                        </p>

                        <div style={{ display: 'grid', gap: '12px' }}>
                            <div>
                                <label style={{ display: 'block', marginBottom: '4px', fontWeight: '500', color: '#374151' }}>
                                    Token Names (comma-separated):
                                </label>
                                <input
                                    placeholder="e.g., USDC,USDT,DAI,FRAX"
                                    value={configTokens}
                                    onChange={e => setConfigTokens(e.target.value)}
                                    style={{ width: '100%', padding: '8px', border: '1px solid #d1d5db', borderRadius: '6px', boxSizing: 'border-box' }}
                                />
                            </div>

                            <div>
                                <label style={{ display: 'block', marginBottom: '4px', fontWeight: '500', color: '#374151' }}>
                                    Initial Reserves (comma-separated):
                                </label>
                                <input
                                    placeholder="e.g., 1000,1500,2000,800"
                                    value={configReserves}
                                    onChange={e => setConfigReserves(e.target.value)}
                                    style={{ width: '100%', padding: '8px', border: '1px solid #d1d5db', borderRadius: '6px', boxSizing: 'border-box' }}
                                />
                            </div>

                            <div>
                                <label style={{ display: 'block', marginBottom: '4px', fontWeight: '500', color: '#374151' }}>
                                    Initial Plane Constant:
                                </label>
                                <input
                                    type="number"
                                    placeholder="e.g., 600"
                                    value={configPlane}
                                    onChange={e => setConfigPlane(e.target.value)}
                                    style={{ width: '100%', padding: '8px', border: '1px solid #d1d5db', borderRadius: '6px', boxSizing: 'border-box' }}
                                />
                            </div>

                            <div style={{ display: 'flex', gap: '8px', marginTop: '8px' }}>
                                <button
                                    onClick={reconfigureAMM}
                                    style={{
                                        flex: 1,
                                        padding: '10px',
                                        backgroundColor: '#dc2626',
                                        color: 'white',
                                        border: 'none',
                                        borderRadius: '6px',
                                        fontWeight: '500',
                                        cursor: 'pointer'
                                    }}
                                >
                                    🔄 Reconfigure AMM
                                </button>
                                <button
                                    onClick={() => setShowConfig(false)}
                                    style={{
                                        flex: 1,
                                        padding: '10px',
                                        backgroundColor: '#6b7280',
                                        color: 'white',
                                        border: 'none',
                                        borderRadius: '6px',
                                        fontWeight: '500',
                                        cursor: 'pointer'
                                    }}
                                >
                                    Cancel
                                </button>
                            </div>
                        </div>
                    </div>
                ) : null}

                <div style={{ display: 'grid', gridTemplateColumns: 'repeat(auto-fit, minmax(200px, 1fr))', gap: '16px', fontSize: '14px' }}>
                    <div style={{ padding: '12px', backgroundColor: '#f8fafc', borderRadius: '6px' }}>
                        <strong style={{ color: '#1e293b' }}>Current Tokens:</strong><br />
                        <span style={{ fontFamily: 'monospace', color: '#475569' }}>
                            {state.token_names.join(', ')}
                        </span>
                    </div>
                    <div style={{ padding: '12px', backgroundColor: '#f8fafc', borderRadius: '6px' }}>
                        <strong style={{ color: '#1e293b' }}>Global Reserves:</strong><br />
                        <span style={{ fontFamily: 'monospace', color: '#475569' }}>
                            [{state.global_reserves.map(r => r.toFixed(1)).join(', ')}]
                        </span>
                    </div>
                    <div style={{ padding: '12px', backgroundColor: '#f8fafc', borderRadius: '6px' }}>
                        <strong style={{ color: '#1e293b' }}>Total Ticks:</strong><br />
                        <span style={{ fontFamily: 'monospace', color: '#475569' }}>
                            {state.tick_count}
                        </span>
                    </div>
                </div>
            </div>

            <div style={{
                backgroundColor: 'white',
                padding: '20px',
                borderRadius: '12px',
                marginTop: '20px',
                boxShadow: '0 1px 3px rgba(0,0,0,0.1)'
            }}>
                <h2 style={{ margin: '0 0 12px 0', color: '#1e293b' }}>📚 Quick Guide</h2>
                <div style={{ display: 'grid', gridTemplateColumns: 'repeat(auto-fit, minmax(300px, 1fr))', gap: '16px', fontSize: '14px', color: '#475569' }}>
                    <div>
                        <strong>Ticks:</strong> Liquidity bands with different risk levels. Lower plane constants = tighter liquidity around 1:1.
                    </div>
                    <div>
                        <strong>Interior/Boundary:</strong> Interior ticks are active for trading. Boundary ticks have hit their limits.
                    </div>
                    <div>
                        <strong>Trading:</strong> Routes through ticks automatically, starting with smallest plane constants first.
                    </div>
                    <div>
                        <strong>Sphere Constraint:</strong> Each tick maintains Σ(r - xᵢ)² = r² to ensure mathematical consistency.
                    </div>
                </div>
            </div>
        </div>
    );
}