"use client";

import { useState, useEffect } from "react";
import Link from "next/link";
import { PixelWorld } from "@/components/PixelWorld";
import { PixelCat } from "@/components/PixelCat";
import { PixelChip } from "@/components/PixelChip";

export default function Home() {
  const [tableId, setTableId] = useState(0);
  const [showContent, setShowContent] = useState(false);
  const [pressStart, setPressStart] = useState(false);

  useEffect(() => {
    const timer = setTimeout(() => setShowContent(true), 300);
    return () => clearTimeout(timer);
  }, []);

  if (!pressStart) {
    return (
      <PixelWorld>
        <div className="min-h-screen flex flex-col items-center justify-center gap-6 p-8 cursor-pointer select-none" onClick={() => setPressStart(true)}>
          {/* Hearts row */}
          <div className="flex gap-3 mb-2" style={{
            opacity: showContent ? 1 : 0,
            transition: 'opacity 0.5s ease-in',
            transitionDelay: '0.2s',
          }}>
            <PixelChip color="red" size={5} />
            <PixelChip color="gold" size={5} />
            <PixelChip color="blue" size={5} />
          </div>

          {/* Title */}
          <div className="text-center" style={{
            opacity: showContent ? 1 : 0,
            transform: showContent ? 'translateY(0)' : 'translateY(-20px)',
            transition: 'all 0.6s ease-out',
            transitionDelay: '0.4s',
          }}>
            <h1 className="text-4xl md:text-5xl leading-relaxed" style={{
              color: 'white',
              textShadow: '4px 4px 0 #2c3e50, -1px -1px 0 #2c3e50, 1px -1px 0 #2c3e50, -1px 1px 0 #2c3e50',
              letterSpacing: '3px',
            }}>
              POKER
            </h1>
            <h2 className="text-2xl md:text-3xl mt-1" style={{
              color: 'white',
              textShadow: '3px 3px 0 #2c3e50, -1px -1px 0 #2c3e50, 1px -1px 0 #2c3e50, -1px 1px 0 #2c3e50',
              letterSpacing: '2px',
            }}>
              ON STELLAR
            </h2>
          </div>

          {/* Press to Start */}
          <div
            className="mt-6"
            style={{
              opacity: showContent ? 1 : 0,
              transition: 'opacity 0.5s ease-in',
              transitionDelay: '0.8s',
              animation: showContent ? 'textPulse 1.5s ease-in-out infinite' : undefined,
              color: '#f5e6c8',
              textShadow: '2px 2px 0 #2c3e50',
              fontSize: '14px',
              fontFamily: "'Press Start 2P', monospace",
            }}
          >
            CLICK ANYWHERE TO START
          </div>

          {/* Cats decorating the scene */}
          <div className="fixed bottom-[12%] left-[6%] z-[5]" style={{
            opacity: showContent ? 1 : 0,
            transition: 'opacity 0.5s',
            transitionDelay: '1s',
          }}>
            <PixelCat variant="grey" size={10} />
          </div>
          <div className="fixed bottom-[14%] left-[38%] z-[5]" style={{
            opacity: showContent ? 1 : 0,
            transition: 'opacity 0.5s',
            transitionDelay: '1.2s',
          }}>
            <PixelCat variant="orange" size={12} />
          </div>
          <div className="fixed bottom-[12%] right-[6%] z-[5]" style={{
            opacity: showContent ? 1 : 0,
            transition: 'opacity 0.5s',
            transitionDelay: '1.4s',
          }}>
            <PixelCat variant="black" size={12} flipped />
          </div>
        </div>
      </PixelWorld>
    );
  }

  return (
    <PixelWorld>
      <div className="min-h-screen flex flex-col items-center justify-center gap-8 p-8 relative">
        {/* Back button */}
        <button
          onClick={() => setPressStart(false)}
          className="absolute top-6 left-6 z-20 text-[14px]"
          style={{
            color: '#f5e6c8',
            textShadow: '2px 2px 0 #2c3e50',
            background: 'none',
            border: 'none',
            cursor: 'pointer',
            fontFamily: "'Press Start 2P', monospace",
          }}
        >
          ←
        </button>

        {/* Logo area */}
        <div className="text-center">
          <div className="flex gap-2 justify-center mb-3">
            <PixelChip color="red" size={4} />
            <PixelChip color="gold" size={4} />
            <PixelChip color="blue" size={4} />
          </div>
          <h1 className="text-3xl md:text-4xl leading-relaxed" style={{
            color: 'white',
            textShadow: '3px 3px 0 #2c3e50',
            letterSpacing: '2px',
          }}>
            POKER ON STELLAR
          </h1>
          <p className="text-[9px] mt-3" style={{
            color: '#c8e6ff',
            textShadow: '1px 1px 0 rgba(0,0,0,0.5)',
          }}>
            PRIVATE POKER ON THE BLOCKCHAIN
          </p>
        </div>

        {/* Join table panel */}
        <div className="p-6 flex flex-col items-center gap-5" style={{
          background: 'rgba(12, 10, 24, 0.88)',
          border: '4px solid #c47d2e',
          boxShadow: 'inset -4px -4px 0px 0px rgba(0,0,0,0.3), inset 4px 4px 0px 0px rgba(255,255,255,0.08), 0 4px 0 0 rgba(0,0,0,0.4), 0 0 20px rgba(196, 125, 46, 0.08)',
          minWidth: '320px',
        }}>
          <h2 className="text-xs" style={{
            color: '#ffc078',
            textShadow: '1px 1px 0 rgba(0,0,0,0.6)',
          }}>
            JOIN A TABLE
          </h2>

          <div className="flex items-center gap-3">
            <label className="text-[8px]" style={{ color: '#a0a8b8' }}>TABLE:</label>
            <input
              type="number"
              value={tableId}
              onChange={(e) => setTableId(Number(e.target.value))}
              min={0}
              className="w-16 text-center text-[10px]"
            />
          </div>

          <Link
            href={`/table/${tableId}`}
            className="pixel-btn pixel-btn-green text-[10px]"
          >
            PLAY NOW
          </Link>
        </div>

        {/* Cats at bottom */}
        <div className="fixed bottom-[14%] left-[6%] z-[5]">
          <PixelCat variant="grey" size={4} />
        </div>
        <div className="fixed bottom-[13%] right-[6%] z-[5]">
          <PixelCat variant="orange" size={5} flipped />
        </div>
      </div>
    </PixelWorld>
  );
}

