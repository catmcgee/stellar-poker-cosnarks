"use client";

import { useState, useEffect, useRef, useCallback } from "react";

/**
 * PixelWorld — immersive background with day/night toggle and ambient music.
 * Click the sun to transition to night (crescent moon, stars, dark sky).
 * Click the moon to return to day. Music crossfades with the visual transition.
 */
export function PixelWorld({ children }: { children: React.ReactNode }) {
  const [isNight, setIsNight] = useState(false);
  const dayAudioRef = useRef<HTMLAudioElement | null>(null);
  const nightAudioRef = useRef<HTMLAudioElement | null>(null);
  const fadeRef = useRef<ReturnType<typeof setInterval> | null>(null);
  const musicStartedRef = useRef(false);

  const FADE_MS = 2000; // matches visual transition duration
  const FADE_STEP = 50; // ms per volume tick

  // Create audio elements once on mount
  useEffect(() => {
    const day = new Audio("/music/day-music.mp3");
    day.loop = true;
    day.volume = 1;
    const night = new Audio("/music/night-music.mp3");
    night.loop = true;
    night.volume = 0;
    dayAudioRef.current = day;
    nightAudioRef.current = night;

    return () => {
      day.pause();
      night.pause();
      day.src = "";
      night.src = "";
    };
  }, []);

  // Crossfade when isNight changes
  const crossfade = useCallback((toNight: boolean) => {
    const fadeIn = toNight ? nightAudioRef.current : dayAudioRef.current;
    const fadeOut = toNight ? dayAudioRef.current : nightAudioRef.current;
    if (!fadeIn || !fadeOut) return;

    // Start the incoming track if paused
    fadeIn.play().catch(() => {});

    if (fadeRef.current) clearInterval(fadeRef.current);
    const steps = FADE_MS / FADE_STEP;
    let step = 0;

    fadeRef.current = setInterval(() => {
      step++;
      const progress = Math.min(step / steps, 1);
      fadeIn.volume = Math.min(progress, 1);
      fadeOut.volume = Math.max(1 - progress, 0);

      if (step >= steps) {
        if (fadeRef.current) clearInterval(fadeRef.current);
        fadeRef.current = null;
        fadeOut.pause();
        fadeOut.currentTime = 0;
      }
    }, FADE_STEP);
  }, []);

  // Start music on first user click anywhere in the world
  const handleFirstInteraction = useCallback(() => {
    if (musicStartedRef.current) return;
    musicStartedRef.current = true;
    const active = isNight ? nightAudioRef.current : dayAudioRef.current;
    const inactive = isNight ? dayAudioRef.current : nightAudioRef.current;
    if (active) {
      active.volume = 1;
      active.play().catch(() => {});
    }
    if (inactive) {
      inactive.volume = 0;
    }
  }, [isNight]);

  const duration = '2s';

  return (
    <div className="relative min-h-screen overflow-hidden" onClick={handleFirstInteraction}>
      {/* Day sky */}
      <div className="absolute inset-0" style={{
        background: 'linear-gradient(180deg, #4a90d9 0%, #6bb3e0 40%, #87ceeb 70%, #a8dcf0 100%)',
        opacity: isNight ? 0 : 1,
        transition: `opacity ${duration} ease-in-out`,
      }} />
      {/* Night sky */}
      <div className="absolute inset-0" style={{
        background: 'linear-gradient(180deg, #070b1a 0%, #0f1530 30%, #1a1845 55%, #0d1225 100%)',
        opacity: isNight ? 1 : 0,
        transition: `opacity ${duration} ease-in-out`,
      }} />
      {/* Sun / Moon — click to toggle day/night */}
      <div
        className="absolute top-8 right-16 z-[15] cursor-pointer"
        onClick={(e) => {
          e.stopPropagation();
          handleFirstInteraction();
          const next = !isNight;
          setIsNight(next);
          crossfade(next);
        }}
        title={isNight ? "Switch to day" : "Switch to night"}
        style={{
          width: '64px',
          height: '64px',
          borderRadius: '50%',
          overflow: 'hidden',
          transition: `transform ${duration} cubic-bezier(0.4, 0, 0.2, 1)`,
          transform: isNight ? 'scale(0.9) rotate(-15deg)' : 'scale(1) rotate(0deg)',
        }}
      >
        {/* Base circle (sun or moon glow) */}
        <div style={{
          width: '100%',
          height: '100%',
          borderRadius: '50%',
          background: isNight ? '#e8e8f0' : '#f1c40f',
          position: 'relative',
          overflow: 'hidden',
        }}>
          {/* Dark overlay circle that slides in to create crescent */}
          <div style={{
            position: 'absolute',
            top: '-6px',
            left: isNight ? '16px' : '70px',
            width: '64px',
            height: '76px',
            borderRadius: '50%',
            background: isNight ? '#0f1530' : '#0f1530',
            transition: `left ${duration} cubic-bezier(0.4, 0, 0.2, 1), opacity ${duration} ease-in-out`,
            opacity: isNight ? 1 : 0,
          }} />
        </div>
      </div>

      {/* Stars (fade in at night) */}
      <div className="absolute inset-0 z-[0]" style={{
        opacity: isNight ? 1 : 0,
        transition: `opacity ${duration} ease-in-out`,
        transitionDelay: isNight ? '0.8s' : '0s',
        pointerEvents: 'none',
      }}>
        {[
          { x: 8, y: 6, s: 2, d: 0 }, { x: 22, y: 12, s: 3, d: 0.3 },
          { x: 38, y: 4, s: 2, d: 0.7 }, { x: 52, y: 18, s: 2, d: 1.1 },
          { x: 65, y: 8, s: 3, d: 0.5 }, { x: 78, y: 22, s: 2, d: 1.4 },
          { x: 12, y: 28, s: 2, d: 0.9 }, { x: 32, y: 10, s: 2, d: 1.7 },
          { x: 48, y: 26, s: 3, d: 0.2 }, { x: 88, y: 6, s: 2, d: 1.0 },
          { x: 3, y: 18, s: 2, d: 1.3 }, { x: 72, y: 3, s: 2, d: 0.6 },
          { x: 58, y: 14, s: 2, d: 1.8 }, { x: 42, y: 22, s: 3, d: 0.4 },
          { x: 18, y: 3, s: 2, d: 1.5 }, { x: 95, y: 15, s: 2, d: 0.8 },
          { x: 28, y: 20, s: 2, d: 1.2 }, { x: 82, y: 30, s: 3, d: 0.1 },
        ].map((star, i) => (
          <div key={i} className="absolute" style={{
            left: `${star.x}%`,
            top: `${star.y}%`,
            width: `${star.s}px`,
            height: `${star.s}px`,
            background: '#fff',
            animation: `twinkle ${2 + (i % 3)}s ease-in-out ${star.d}s infinite`,
          }} />
        ))}
      </div>

      {/* Clouds layer */}
      <div style={{
        opacity: isNight ? 0.1 : 0.95,
        filter: isNight ? 'brightness(0.5)' : 'none',
        transition: `opacity ${duration} ease-in-out, filter ${duration} ease-in-out`,
      }}>
        <PixelCloud top={60} delay={0} speed={45} size={1.2} />
        <PixelCloud top={30} delay={12} speed={55} size={0.9} />
        <PixelCloud top={100} delay={25} speed={38} size={1.0} />
        <PixelCloud top={140} delay={8} speed={50} size={0.7} />
        <PixelCloud top={80} delay={35} speed={60} size={1.1} />
      </div>

      {/* Far hills */}
      <div className="absolute bottom-0 left-0 right-0 z-[1]" style={{
        height: '30%',
        filter: isNight ? 'brightness(0.2) saturate(0.3)' : 'none',
        transition: `filter ${duration} ease-in-out`,
      }}>
        <svg viewBox="0 0 1200 200" preserveAspectRatio="none" className="w-full h-full">
          <defs>
            <pattern id="farGrass" width="128" height="128" patternUnits="userSpaceOnUse">
              {grassTiles(['#5cb85c','#4cae4c','#68c468','#489848','#55b055','#6ed66e','#3d8b3d'], 8, 16, 3)}
            </pattern>
          </defs>
          <path d="M0,120 Q150,40 300,100 Q450,50 600,90 Q750,30 900,80 Q1050,50 1200,70 L1200,200 L0,200 Z"
                fill="url(#farGrass)" />
        </svg>
      </div>

      {/* Mid hills */}
      <div className="absolute bottom-0 left-0 right-0 z-[2]" style={{
        height: '22%',
        filter: isNight ? 'brightness(0.18) saturate(0.3)' : 'none',
        transition: `filter ${duration} ease-in-out`,
      }}>
        <svg viewBox="0 0 1200 160" preserveAspectRatio="none" className="w-full h-full">
          <defs>
            <pattern id="midGrass" width="128" height="128" patternUnits="userSpaceOnUse">
              {grassTiles(['#4cae4c','#3d8b3d','#5cb85c','#2d6b2d','#45a845','#6ed66e','#358435','#8bc34a'], 8, 16, 7)}
            </pattern>
          </defs>
          <path d="M0,80 Q100,30 250,70 Q400,20 550,60 Q700,10 850,55 Q1000,25 1200,50 L1200,160 L0,160 Z"
                fill="url(#midGrass)" />
        </svg>
      </div>

      {/* Foreground grass */}
      <div className="absolute bottom-0 left-0 right-0 z-[3]" style={{
        height: '12%',
        filter: isNight ? 'brightness(0.18) saturate(0.4)' : 'none',
        transition: `filter ${duration} ease-in-out`,
      }}>
        <svg viewBox="0 0 1200 100" preserveAspectRatio="none" className="w-full h-full">
          <defs>
            <pattern id="fgGrass" width="128" height="128" patternUnits="userSpaceOnUse">
              {grassTiles(['#3d8b3d','#2d6b2d','#4cae4c','#27ae60','#358535','#5cb85c','#1e7a2e','#45a845'], 8, 16, 11)}
            </pattern>
          </defs>
          <rect width="1200" height="100" fill="url(#fgGrass)" />
        </svg>
      </div>

      {/* Decorative bushes & flowers */}
      <div style={{
        filter: isNight ? 'brightness(0.18) saturate(0.3)' : 'none',
        transition: `filter ${duration} ease-in-out`,
      }}>
        <PixelBush left="5%" bottom="11%" />
        <PixelBush left="85%" bottom="10%" />
        <PixelBush left="45%" bottom="11.5%" />
        <PixelFlower left="15%" bottom="12%" color="#e74c3c" />
        <PixelFlower left="30%" bottom="13%" color="#f1c40f" />
        <PixelFlower left="70%" bottom="12.5%" color="#e74c3c" />
        <PixelFlower left="90%" bottom="13%" color="#9b59b6" />
      </div>

      {/* Content layer */}
      <div className="relative z-[10]">
        {children}
      </div>
    </div>
  );
}

/* Deterministic mosaic tile generator for grass/hills.
 * Uses larger blocks and clumps adjacent tiles to the same color
 * so the result looks organic rather than noisy. */
function grassTiles(colors: string[], blockSize: number, gridSize: number, seed: number) {
  // Pre-compute a color grid with large organic patches.
  // High neighbor-copy probability creates natural-looking clumps.
  const grid: number[][] = [];
  for (let y = 0; y < gridSize; y++) {
    grid[y] = [];
    for (let x = 0; x < gridSize; x++) {
      const hash = ((x * 11 + y * 17 + x * y * 5 + seed) * 31 + seed * 7) & 0xffff;
      const roll = hash % 100;
      // ~60% copy left, ~22% copy above, ~8% copy diagonal — only ~10% picks a new color
      if (x > 0 && roll < 60) {
        grid[y][x] = grid[y][x - 1];
      } else if (y > 0 && roll < 82) {
        grid[y][x] = grid[y - 1][x];
      } else if (x > 0 && y > 0 && roll < 90) {
        grid[y][x] = grid[y - 1][x - 1];
      } else {
        grid[y][x] = ((hash >> 3) % colors.length + colors.length) % colors.length;
      }
    }
  }

  const rects = [];
  for (let y = 0; y < gridSize; y++) {
    for (let x = 0; x < gridSize; x++) {
      rects.push(
        <rect key={`${x}-${y}`} x={x * blockSize} y={y * blockSize}
              width={blockSize} height={blockSize} fill={colors[grid[y][x]]} />
      );
    }
  }
  return rects;
}

function PixelCloud({ top, delay, speed, size }: { top: number; delay: number; speed: number; size: number }) {
  const p = 8;
  const c: Record<string, string> = { w: '#fff', l: '#dde8f0', s: '#b8ccdc' };
  const shape = [
    '          ww                         ',
    '        wwwwww          ww           ',
    '       wwwwwwww       wwwwww         ',
    '      wwwwwwwwwww   wwwwwwwww        ',
    '     wwwwwwwwwwwww wwwwwwwwwww       ',
    '    wwwwwwwwwwwwwwwwwwwwwwwwwww      ',
    '   wwwwwwwwwwwwwwwwwwwwwwwwwwwww     ',
    '  wwwwwwwwwwwwwwwwwwwwwwwwwwwwwww    ',
    '  lwwwwwwwwwwwwwwwwwwwwwwwwwwwwwl    ',
    '  lllllwwwwwwwwwwwwwwwwwwwwwllll     ',
    '   ssllllllllllllllllllllllss        ',
    '     sssssssssssssssssssss           ',
  ];

  const shadows: string[] = [];
  shape.forEach((row, y) => {
    for (let x = 0; x < row.length; x++) {
      const ch = row[x];
      if (c[ch]) shadows.push(`${x * p}px ${y * p}px 0 0.5px ${c[ch]}`);
    }
  });

  return (
    <div className="absolute z-[0]" style={{
      top: `${top}px`,
      left: '-300px',
      animation: `cloudFloat2 ${speed}s linear ${delay}s infinite`,
    }}>
      <div style={{ transform: `scale(${size})` }}>
        <div style={{
          width: `${p}px`,
          height: `${p}px`,
          background: 'transparent',
          boxShadow: shadows.join(', '),
        }} />
      </div>
    </div>
  );
}

function PixelBush({ left, bottom }: { left: string; bottom: string }) {
  return (
    <div className="absolute z-[4]" style={{ left, bottom }}>
      <div style={{
        width: '6px',
        height: '6px',
        background: '#2d8b3d',
        boxShadow: `
          6px 0 0 #2d8b3d, 12px 0 0 #2d8b3d,
          -6px 6px 0 #1e7a2e, 0 6px 0 #27ae60, 6px 6px 0 #2ecc71, 12px 6px 0 #27ae60, 18px 6px 0 #1e7a2e,
          -6px 12px 0 #1e7a2e, 0 12px 0 #27ae60, 6px 12px 0 #2ecc71, 12px 12px 0 #27ae60, 18px 12px 0 #1e7a2e,
          0 18px 0 #1e7a2e, 6px 18px 0 #27ae60, 12px 18px 0 #1e7a2e
        `,
      }} />
    </div>
  );
}

function PixelFlower({ left, bottom, color }: { left: string; bottom: string; color: string }) {
  return (
    <div className="absolute z-[4]" style={{ left, bottom }}>
      <div style={{
        width: '4px',
        height: '4px',
        background: '#27ae60',
        boxShadow: `
          0 -4px 0 ${color}, 4px 0 0 ${color}, 0 4px 0 #27ae60, -4px 0 0 ${color},
          0 -8px 0 ${color},
          0 4px 0 #1e7a2e, 0 8px 0 #1e7a2e
        `,
      }} />
    </div>
  );
}
