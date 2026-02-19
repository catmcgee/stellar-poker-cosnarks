"use client";

import { useState } from "react";
import Link from "next/link";

export default function Home() {
  const [tableId, setTableId] = useState(1);

  return (
    <div className="min-h-screen flex flex-col items-center justify-center gap-8 p-8">
      <div className="text-center">
        <h1 className="text-5xl font-bold text-white mb-2">Stellar Poker</h1>
        <p className="text-gray-400 text-lg">
          Onchain Texas Hold&apos;em with private cards
        </p>
        <p className="text-gray-500 text-sm mt-1">
          Powered by TACEO coNoir MPC + UltraHonk ZK proofs on Soroban
        </p>
      </div>

      <div className="flex flex-col items-center gap-4 bg-gray-800/60 rounded-2xl p-8 border border-gray-700">
        <h2 className="text-lg font-semibold text-gray-200">Join a Table</h2>

        <div className="flex items-center gap-3">
          <label className="text-gray-400 text-sm">Table ID:</label>
          <input
            type="number"
            value={tableId}
            onChange={(e) => setTableId(Number(e.target.value))}
            min={1}
            className="w-20 px-3 py-2 bg-gray-700 border border-gray-600 rounded-lg text-white text-center"
          />
        </div>

        <Link
          href={`/table/${tableId}`}
          className="px-8 py-3 bg-green-600 hover:bg-green-500 text-white rounded-lg font-bold text-lg transition shadow-lg"
        >
          Play Now
        </Link>

        <div className="text-xs text-gray-500 text-center mt-4 max-w-sm">
          No single party sees your cards. The MPC committee (3 nodes running
          TACEO coNoir) shuffles and deals using REP3 secret sharing. ZK proofs
          verify every action on-chain.
        </div>
      </div>

      <div className="grid grid-cols-3 gap-6 text-center max-w-2xl">
        <div className="bg-gray-800/40 rounded-xl p-4 border border-gray-700/50">
          <div className="text-2xl mb-2">&#x1f512;</div>
          <div className="text-sm font-medium text-gray-300">Private Cards</div>
          <div className="text-xs text-gray-500 mt-1">
            REP3 MPC ensures no one sees your hand
          </div>
        </div>
        <div className="bg-gray-800/40 rounded-xl p-4 border border-gray-700/50">
          <div className="text-2xl mb-2">&#x2714;</div>
          <div className="text-sm font-medium text-gray-300">ZK Verified</div>
          <div className="text-xs text-gray-500 mt-1">
            UltraHonk proofs verify deals and reveals
          </div>
        </div>
        <div className="bg-gray-800/40 rounded-xl p-4 border border-gray-700/50">
          <div className="text-2xl mb-2">&#x26d3;</div>
          <div className="text-sm font-medium text-gray-300">On-Chain</div>
          <div className="text-xs text-gray-500 mt-1">
            Soroban contracts settle bets trustlessly
          </div>
        </div>
      </div>
    </div>
  );
}
