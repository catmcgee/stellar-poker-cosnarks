"use client";

/**
 * PixelWorld — the immersive background layer.
 * Renders sky gradient, animated pixel clouds, rolling pixel hills,
 * grass texture, and decorative pixel flowers/bushes.
 */
export function PixelWorld({ children }: { children: React.ReactNode }) {
  return (
    <div className="relative min-h-screen overflow-hidden" style={{
      background: 'linear-gradient(180deg, #4a90d9 0%, #6bb3e0 40%, #87ceeb 70%, #a8dcf0 100%)'
    }}>
      {/* Sun */}
      <div className="absolute top-8 right-16 w-16 h-16 z-0" style={{
        background: '#f1c40f',
        boxShadow: '0 0 40px 15px rgba(241,196,15,0.3), 0 0 80px 30px rgba(241,196,15,0.15)',
        imageRendering: 'pixelated',
      }} />

      {/* Clouds layer */}
      <PixelCloud top={60} delay={0} speed={45} size={1.2} />
      <PixelCloud top={30} delay={12} speed={55} size={0.9} />
      <PixelCloud top={100} delay={25} speed={38} size={1.0} />
      <PixelCloud top={140} delay={8} speed={50} size={0.7} />
      <PixelCloud top={80} delay={35} speed={60} size={1.1} />

      {/* Far hills (lighter, behind) */}
      <div className="absolute bottom-0 left-0 right-0 z-[1]" style={{ height: '30%' }}>
        <svg viewBox="0 0 1200 200" preserveAspectRatio="none" className="w-full h-full">
          <path d="M0,120 Q150,40 300,100 Q450,50 600,90 Q750,30 900,80 Q1050,50 1200,70 L1200,200 L0,200 Z"
                fill="#5cb85c" />
        </svg>
      </div>

      {/* Mid hills (darker, in front) */}
      <div className="absolute bottom-0 left-0 right-0 z-[2]" style={{ height: '22%' }}>
        <svg viewBox="0 0 1200 160" preserveAspectRatio="none" className="w-full h-full">
          <path d="M0,80 Q100,30 250,70 Q400,20 550,60 Q700,10 850,55 Q1000,25 1200,50 L1200,160 L0,160 Z"
                fill="#4cae4c" />
        </svg>
      </div>

      {/* Foreground grass */}
      <div className="absolute bottom-0 left-0 right-0 z-[3]" style={{ height: '12%' }}>
        <div className="w-full h-full" style={{
          background: `
            repeating-linear-gradient(90deg, #3d8b3d 0px, #4cae4c 4px, #3d8b3d 8px, #2d6b2d 12px, #3d8b3d 16px),
            linear-gradient(180deg, #4cae4c 0%, #2d6b2d 100%)
          `,
          backgroundBlendMode: 'multiply',
        }}>
          {/* Grass blade pattern */}
          <div className="w-full h-3 relative overflow-hidden" style={{
            background: 'repeating-linear-gradient(90deg, transparent 0px, transparent 3px, #5cb85c 3px, #5cb85c 5px, transparent 5px, transparent 10px)',
          }} />
        </div>
      </div>

      {/* Decorative pixel bushes */}
      <PixelBush left="5%" bottom="11%" />
      <PixelBush left="85%" bottom="10%" />
      <PixelBush left="45%" bottom="11.5%" />

      {/* Small pixel flowers */}
      <PixelFlower left="15%" bottom="12%" color="#e74c3c" />
      <PixelFlower left="30%" bottom="13%" color="#f1c40f" />
      <PixelFlower left="70%" bottom="12.5%" color="#e74c3c" />
      <PixelFlower left="90%" bottom="13%" color="#9b59b6" />

      {/* Content layer */}
      <div className="relative z-[10]">
        {children}
      </div>
    </div>
  );
}

function PixelCloud({ top, delay, speed, size }: { top: number; delay: number; speed: number; size: number }) {
  return (
    <div
      className="absolute z-[0]"
      style={{
        top: `${top}px`,
        left: '-200px',
        animation: `cloudFloat2 ${speed}s linear ${delay}s infinite`,
        transform: `scale(${size})`,
      }}
    >
      {/* Pixel cloud using box-shadow technique */}
      <div style={{
        width: '8px',
        height: '8px',
        background: 'white',
        boxShadow: `
          8px 0 0 white, 16px 0 0 white, 24px 0 0 white,
          -8px 8px 0 white, 0 8px 0 white, 8px 8px 0 white, 16px 8px 0 white, 24px 8px 0 white, 32px 8px 0 white,
          -16px 16px 0 white, -8px 16px 0 white, 0 16px 0 white, 8px 16px 0 white, 16px 16px 0 white, 24px 16px 0 white, 32px 16px 0 white, 40px 16px 0 white,
          -16px 24px 0 white, -8px 24px 0 white, 0 24px 0 white, 8px 24px 0 white, 16px 24px 0 white, 24px 24px 0 white, 32px 24px 0 white, 40px 24px 0 white,
          -8px 32px 0 white, 0 32px 0 white, 8px 32px 0 white, 16px 32px 0 white, 24px 32px 0 white, 32px 32px 0 white,

          56px 0 0 white, 64px 0 0 white,
          48px 8px 0 white, 56px 8px 0 white, 64px 8px 0 white, 72px 8px 0 white,
          48px 16px 0 white, 56px 16px 0 white, 64px 16px 0 white, 72px 16px 0 white, 80px 16px 0 white,
          48px 24px 0 white, 56px 24px 0 white, 64px 24px 0 white, 72px 24px 0 white, 80px 24px 0 white,
          48px 32px 0 white, 56px 32px 0 white, 64px 32px 0 white, 72px 32px 0 white
        `,
        opacity: 0.95,
      }} />
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
