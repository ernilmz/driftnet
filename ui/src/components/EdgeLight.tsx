import React, { useEffect, useRef } from "react";
export function EdgeLight({
  active,
  fast,
}: {
  active: boolean;
  fast: boolean;
}) {
  const canvas = useRef<HTMLCanvasElement>(null);
  useEffect(() => {
    const element = canvas.current;
    if (!element) return;
    const context = element.getContext("2d");
    if (!context) return;
    const motion = matchMedia("(prefers-reduced-motion: reduce)");
    let frame = 0;
    let visible = !document.hidden;
    let points: { x: number; y: number }[] = [];
    let width = 0;
    let height = 0;
    const measure = () => {
      const rect = element.getBoundingClientRect();
      width = rect.width;
      height = rect.height;
      const dpr = Math.min(devicePixelRatio, 2);
      element.width = Math.round(width * dpr);
      element.height = Math.round(height * dpr);
      context.setTransform(dpr, 0, 0, dpr, 0, 0);
      points = [];
      const r = 19,
        p = 1.2;
      for (let side = 0; side < 4; side++) {
        const start =
          side === 0
            ? { x: r, y: p }
            : side === 1
              ? { x: width - p, y: r }
              : side === 2
                ? { x: width - r, y: height - p }
                : { x: p, y: height - r };
        const end =
          side === 0
            ? { x: width - r, y: p }
            : side === 1
              ? { x: width - p, y: height - r }
              : side === 2
                ? { x: r, y: height - p }
                : { x: p, y: r };
        const steps = Math.max(
          2,
          Math.round(Math.hypot(end.x - start.x, end.y - start.y) / 3),
        );
        for (let i = 0; i < steps; i++)
          points.push({
            x: start.x + ((end.x - start.x) * i) / steps,
            y: start.y + ((end.y - start.y) * i) / steps,
          });
        const cx = side === 0 || side === 1 ? width - r : r,
          cy = side === 0 || side === 3 ? r : height - r;
        const angle = -Math.PI / 2 + (side * Math.PI) / 2;
        for (let i = 0; i < 12; i++) {
          const t = angle + ((i / 12) * Math.PI) / 2;
          points.push({
            x: cx + Math.cos(t) * (r - p),
            y: cy + Math.sin(t) * (r - p),
          });
        }
      }
    };
    const draw = (time: number) => {
      context.clearRect(0, 0, width, height);
      if (active && visible && !motion.matches && points.length) {
        const head = Math.floor(
          ((time / (fast ? 1800 : 4200)) % 1) * points.length,
        );
        const trail = Math.floor(points.length * 0.2);
        const light = points[head];
        const halo = context.createRadialGradient(
          light.x,
          light.y,
          0,
          light.x,
          light.y,
          65,
        );
        halo.addColorStop(0, "rgba(255,183,73,.16)");
        halo.addColorStop(0.45, "rgba(245,151,42,.045)");
        halo.addColorStop(1, "rgba(245,151,42,0)");
        context.fillStyle = halo;
        context.fillRect(0, 0, width, height);
        for (let i = 0; i < trail; i++) {
          const first = points[(head - i + points.length) % points.length],
            next = points[(head - i - 1 + points.length) % points.length];
          const alpha = Math.pow(1 - i / trail, 1.8) * 0.95;
          context.beginPath();
          context.moveTo(first.x, first.y);
          context.lineTo(next.x, next.y);
          context.lineWidth = 1.6;
          context.strokeStyle = `rgba(255,197,104,${alpha})`;
          context.shadowColor = `rgba(247,162,50,${alpha})`;
          context.shadowBlur = 12;
          context.stroke();
        }
        context.shadowBlur = 0;
      }
      if (active && visible && !motion.matches)
        frame = requestAnimationFrame(draw);
    };
    const restart = () => {
      cancelAnimationFrame(frame);
      measure();
      draw(performance.now());
    };
    const visibility = () => {
      visible = !document.hidden;
      restart();
    };
    const observer = new ResizeObserver(restart);
    observer.observe(element);
    motion.addEventListener("change", restart);
    document.addEventListener("visibilitychange", visibility);
    restart();
    return () => {
      cancelAnimationFrame(frame);
      observer.disconnect();
      motion.removeEventListener("change", restart);
      document.removeEventListener("visibilitychange", visibility);
      context.clearRect(0, 0, width, height);
    };
  }, [active, fast]);
  return <canvas ref={canvas} className="edge-light" aria-hidden="true" />;
}
