// @ts-nocheck
import React, { useEffect, useState, useRef } from 'react';

interface PhasePoint {
    x1: number;
    x2: number;
    parallel_magnitude: number;
    distance_from_equilibrium: number;
    is_valid: boolean;
}

interface TickData {
    parallel_magnitude: number;
    plane_constant: number;
    reserves: number[];
    is_interior: boolean;
    is_boundary: boolean;
}

interface PhaseData {
    phase_points: PhasePoint[];
    equal_price_point: number;
    radius: number;
    current_ticks: TickData[];
}

const PhaseDiagram: React.FC = () => {
    const canvasRef = useRef<HTMLCanvasElement>(null);
    const [phaseData, setPhaseData] = useState<PhaseData | null>(null);
    const [loading, setLoading] = useState(true);

    const fetchPhaseData = async () => {
        try {
            const response = await fetch('/api/phase-diagram');
            if (response.ok) {
                const data = await response.json();
                setPhaseData(data);
            }
        } catch (error) {
            console.error('Failed to fetch phase data:', error);
        } finally {
            setLoading(false);
        }
    };

    useEffect(() => {
        fetchPhaseData();
        const interval = setInterval(fetchPhaseData, 3000);
        return () => clearInterval(interval);
    }, []);

    useEffect(() => {
        if (!phaseData || !canvasRef.current) return;

        const canvas = canvasRef.current;
        const ctx = canvas.getContext('2d');
        if (!ctx) return;

        // Set canvas size
        canvas.width = 600;
        canvas.height = 600;

        // Clear canvas
        ctx.fillStyle = '#f8f9fa';
        ctx.fillRect(0, 0, canvas.width, canvas.height);

        drawPhaseDiagram(ctx, phaseData);
    }, [phaseData]);

    const drawPhaseDiagram = (ctx: CanvasRenderingContext2D, data: PhaseData) => {
        const { width, height } = ctx.canvas;
        const margin = 50;
        const plotWidth = width - 2 * margin;
        const plotHeight = height - 2 * margin;

        // Find data bounds
        const maxReserve = Math.max(
            ...data.phase_points.map(p => Math.max(p.x1, p.x2)),
            ...data.current_ticks.flatMap(t => t.reserves)
        );
        const minReserve = Math.min(
            ...data.phase_points.map(p => Math.min(p.x1, p.x2)),
            ...data.current_ticks.flatMap(t => t.reserves)
        );

        const scaleX = (x: number) => margin + ((x - minReserve) / (maxReserve - minReserve)) * plotWidth;
        const scaleY = (y: number) => height - margin - ((y - minReserve) / (maxReserve - minReserve)) * plotHeight;

        // Draw grid
        ctx.strokeStyle = '#e0e0e0';
        ctx.lineWidth = 1;
        for (let i = 0; i <= 10; i++) {
            const x = margin + (i / 10) * plotWidth;
            const y = margin + (i / 10) * plotHeight;

            ctx.beginPath();
            ctx.moveTo(x, margin);
            ctx.lineTo(x, height - margin);
            ctx.stroke();

            ctx.beginPath();
            ctx.moveTo(margin, y);
            ctx.lineTo(width - margin, y);
            ctx.stroke();
        }

        // Draw phase points
        data.phase_points.forEach(point => {
            if (!point.is_valid) return;

            const x = scaleX(point.x1);
            const y = scaleY(point.x2);

            // Color based on distance from equilibrium
            const distance = point.distance_from_equilibrium;
            const normalizedDistance = Math.min(Math.abs(distance) / (data.radius * 0.5), 1);

            if (Math.abs(distance) < data.radius * 0.05) {
                // Near equilibrium - bright green
                ctx.fillStyle = `rgba(0, 255, 0, 0.8)`;
            } else if (distance > 0) {
                // Above equilibrium - blue gradient
                ctx.fillStyle = `rgba(0, 0, 255, ${0.3 + normalizedDistance * 0.5})`;
            } else {
                // Below equilibrium - red gradient
                ctx.fillStyle = `rgba(255, 0, 0, ${0.3 + normalizedDistance * 0.5})`;
            }

            ctx.beginPath();
            ctx.arc(x, y, 2, 0, 2 * Math.PI);
            ctx.fill();
        });

        // Draw equal-price line
        ctx.strokeStyle = '#00ff00';
        ctx.lineWidth = 3;
        ctx.setLineDash([10, 5]);
        ctx.beginPath();
        ctx.moveTo(margin, scaleY(data.equal_price_point));
        ctx.lineTo(width - margin, scaleY(data.equal_price_point));
        ctx.stroke();
        ctx.setLineDash([]);

        // Draw current tick positions
        data.current_ticks.forEach((tick, index) => {
            const x = scaleX(tick.reserves[0]);
            const y = scaleY(tick.reserves[1]);

            // Tick color based on state
            ctx.fillStyle = tick.is_interior ? '#ff6b35' : tick.is_boundary ? '#f7931e' : '#666';
            ctx.strokeStyle = '#000';
            ctx.lineWidth = 2;

            ctx.beginPath();
            ctx.arc(x, y, 8, 0, 2 * Math.PI);
            ctx.fill();
            ctx.stroke();

            // Label
            ctx.fillStyle = '#000';
            ctx.font = '12px Arial';
            ctx.fillText(`T${index}`, x + 12, y - 12);

            // Draw plane constant line (simplified as horizontal line)
            ctx.strokeStyle = tick.is_interior ? '#ff6b35' : '#f7931e';
            ctx.lineWidth = 2;
            ctx.setLineDash([5, 5]);
            ctx.beginPath();
            ctx.moveTo(margin, scaleY(tick.plane_constant));
            ctx.lineTo(width - margin, scaleY(tick.plane_constant));
            ctx.stroke();
            ctx.setLineDash([]);
        });

        // Draw axes labels
        ctx.fillStyle = '#000';
        ctx.font = '14px Arial';
        ctx.fillText('Token 1 Reserves', width / 2 - 50, height - 10);

        ctx.save();
        ctx.translate(15, height / 2);
        ctx.rotate(-Math.PI / 2);
        ctx.fillText('Token 2 Reserves', -50, 0);
        ctx.restore();

        // Draw legend
        drawLegend(ctx);
    };

    const drawLegend = (ctx: CanvasRenderingContext2D) => {
        const legendX = 20;
        const legendY = 20;

        ctx.fillStyle = 'rgba(255, 255, 255, 0.9)';
        ctx.fillRect(legendX, legendY, 200, 160);
        ctx.strokeStyle = '#000';
        ctx.lineWidth = 1;
        ctx.strokeRect(legendX, legendY, 200, 160);

        ctx.fillStyle = '#000';
        ctx.font = '12px Arial';

        let y = legendY + 20;
        ctx.fillText('Phase Diagram Legend:', legendX + 10, y);

        y += 25;
        ctx.fillStyle = '#00ff00';
        ctx.fillRect(legendX + 10, y - 10, 10, 10);
        ctx.fillStyle = '#000';
        ctx.fillText('Equal-price surface', legendX + 30, y);

        y += 20;
        ctx.fillStyle = 'rgba(0, 0, 255, 0.6)';
        ctx.fillRect(legendX + 10, y - 10, 10, 10);
        ctx.fillStyle = '#000';
        ctx.fillText('Above equilibrium', legendX + 30, y);

        y += 20;
        ctx.fillStyle = 'rgba(255, 0, 0, 0.6)';
        ctx.fillRect(legendX + 10, y - 10, 10, 10);
        ctx.fillStyle = '#000';
        ctx.fillText('Below equilibrium', legendX + 30, y);

        y += 20;
        ctx.fillStyle = '#ff6b35';
        ctx.beginPath();
        ctx.arc(legendX + 15, y - 5, 5, 0, 2 * Math.PI);
        ctx.fill();
        ctx.fillStyle = '#000';
        ctx.fillText('Interior tick', legendX + 30, y);

        y += 20;
        ctx.fillStyle = '#f7931e';
        ctx.beginPath();
        ctx.arc(legendX + 15, y - 5, 5, 0, 2 * Math.PI);
        ctx.fill();
        ctx.fillStyle = '#000';
        ctx.fillText('Boundary tick', legendX + 30, y);
    };

    if (loading) {
        return <div style={{ padding: 20 }}>Loading phase diagram...</div>;
    }

    if (!phaseData) {
        return <div style={{ padding: 20 }}>No phase data available. Add some ticks first!</div>;
    }

    return (
        <div style={{ padding: 20 }}>
            <h3>Phase Diagram: Interior vs Boundary Regions</h3>
            <canvas
                ref={canvasRef}
                style={{ border: '1px solid #ddd', borderRadius: '4px' }}
            />
            <div style={{ marginTop: 20, fontSize: '14px', color: '#666' }}>
                <p><strong>Equal-price point:</strong> {phaseData.equal_price_point.toFixed(2)}</p>
                <p><strong>Sphere radius:</strong> {phaseData.radius.toFixed(2)}</p>
                <p>Green line shows where all tokens have equal relative prices (1:1 equilibrium)</p>
                <p>Tick positions show current liquidity distribution across the phase space</p>
            </div>
        </div>
    );
};

export default PhaseDiagram;