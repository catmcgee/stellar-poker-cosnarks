"use client";

import { useState, useEffect } from "react";
import Link from "next/link";
import { PixelWorld } from "@/components/PixelWorld";
import { PixelCat, PixelHeart } from "@/components/PixelCat";

export default function Home() {
  const [tableId, setTableId] = useState(1);
  const [showContent, setShowContent] = useState(false);
  const [pressStart, setPressStart] = useState(false);

  useEffect(() => {
    const timer = setTimeout(() => setShowContent(true), 300);
    return () => clearTimeout(timer);
  }, []);

  if (!pressStart) {
    return (
      <PixelWorld>
        <div className="min-h-screen flex flex-col items-center justify-center gap-6 p-8">
          {/* Hearts row */}
          <div className="flex gap-3 mb-2" style={{
            opacity: showContent ? 1 : 0,
            transition: 'opacity 0.5s ease-in',
            transitionDelay: '0.2s',
          }}>
            <PixelHeart size={5} beating />
            <PixelHeart size={5} beating />
            <PixelHeart size={5} beating />
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
          <button
            onClick={() => setPressStart(true)}
            className="mt-6"
            style={{
              opacity: showContent ? 1 : 0,
              transition: 'opacity 0.5s ease-in',
              transitionDelay: '0.8s',
              animation: showContent ? 'textPulse 1.5s ease-in-out infinite' : undefined,
              color: '#f5e6c8',
              textShadow: '2px 2px 0 #2c3e50',
              fontSize: '14px',
              background: 'none',
              border: 'none',
              cursor: 'pointer',
              fontFamily: "'Press Start 2P', monospace",
            }}
          >
            PRESS TO START
          </button>

          {/* Cats decorating the scene */}
          <div className="fixed bottom-[14%] left-[8%] z-[5]" style={{
            opacity: showContent ? 1 : 0,
            transition: 'opacity 0.5s',
            transitionDelay: '1s',
          }}>
            <PixelCat variant="grey" size={5} />
          </div>
          <div className="fixed bottom-[15%] left-[35%] z-[5]" style={{
            opacity: showContent ? 1 : 0,
            transition: 'opacity 0.5s',
            transitionDelay: '1.2s',
          }}>
            <PixelCat variant="orange" size={6} />
          </div>
          <div className="fixed bottom-[13%] right-[8%] z-[5]" style={{
            opacity: showContent ? 1 : 0,
            transition: 'opacity 0.5s',
            transitionDelay: '1.4s',
          }}>
            <PixelCat variant="black" size={6} flipped />
          </div>
        </div>
      </PixelWorld>
    );
  }

  return (
    <PixelWorld>
      <div className="min-h-screen flex flex-col items-center justify-center gap-8 p-8">
        {/* Logo area */}
        <div className="text-center">
          <div className="flex gap-2 justify-center mb-3">
            <PixelHeart size={4} beating />
            <PixelHeart size={4} beating />
            <PixelHeart size={4} beating />
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
            PRIVATE CARDS VIA MPC + ZK PROOFS
          </p>
        </div>

        {/* Join table panel */}
        <div className="pixel-border p-6 flex flex-col items-center gap-5" style={{
          background: 'var(--ui-panel)',
          minWidth: '320px',
        }}>
          <h2 className="text-xs" style={{
            color: '#f1c40f',
            textShadow: '1px 1px 0 rgba(0,0,0,0.6)',
          }}>
            JOIN A TABLE
          </h2>

          <div className="flex items-center gap-3">
            <label className="text-[8px]" style={{ color: '#bdc3c7' }}>TABLE:</label>
            <input
              type="number"
              value={tableId}
              onChange={(e) => setTableId(Number(e.target.value))}
              min={1}
              className="w-16 text-center text-[10px]"
            />
          </div>

          <Link
            href={`/table/${tableId}`}
            className="pixel-btn pixel-btn-green text-[10px]"
          >
            PLAY NOW
          </Link>

          <div className="text-[7px] text-center leading-relaxed max-w-xs" style={{ color: '#7f8c8d' }}>
            NO SINGLE PARTY SEES YOUR CARDS.
            MPC COMMITTEE SHUFFLES AND DEALS
            USING REP3 SECRET SHARING.
          </div>
        </div>

        {/* Feature cards */}
        <div className="flex flex-wrap gap-4 justify-center max-w-2xl">
          <FeatureCard
            icon={<PixelCat variant="grey" size={3} />}
            title="PRIVATE"
            desc="REP3 MPC HIDES YOUR HAND"
          />
          <FeatureCard
            icon={<PixelHeart size={3} />}
            title="ZK VERIFIED"
            desc="ULTRAHONK PROOFS ON-CHAIN"
          />
          <FeatureCard
            icon={<PixelCat variant="black" size={3} flipped />}
            title="ON-CHAIN"
            desc="SOROBAN SETTLES BETS"
          />
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

function FeatureCard({ icon, title, desc }: { icon: React.ReactNode; title: string; desc: string }) {
  return (
    <div className="pixel-border-thin p-4 flex flex-col items-center gap-2 w-36" style={{
      background: 'rgba(20, 12, 8, 0.75)',
    }}>
      <div className="mb-1">{icon}</div>
      <div className="text-[8px]" style={{ color: '#f1c40f' }}>{title}</div>
      <div className="text-[6px] text-center leading-relaxed" style={{ color: '#95a5a6' }}>{desc}</div>
    </div>
  );
}
