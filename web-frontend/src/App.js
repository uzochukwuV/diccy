import React, { useEffect, useMemo, useState } from 'react';
import './App.css';
 import * as linera from '@linera/client';

function DiceIcon({ className }) {
  return (
    <svg className={className} fill="none" viewBox="0 0 48 48" xmlns="http://www.w3.org/2000/svg">
      <path d="M24 45.8096C19.6865 45.8096 15.4698 44.5305 11.8832 42.134C8.29667 39.7376 5.50128 36.3314 3.85056 32.3462C2.19985 28.361 1.76794 23.9758 2.60947 19.7452C3.451 15.5145 5.52816 11.6284 8.57829 8.5783C11.6284 5.52817 15.5145 3.45101 19.7452 2.60948C23.9758 1.76795 28.361 2.19986 32.3462 3.85057C36.3314 5.50129 39.7376 8.29668 42.134 11.8833C44.5305 15.4698 45.8096 19.6865 45.8096 24L24 24L24 45.8096Z" fill="currentColor"></path>
    </svg>
  );
}

function Avatar({ url, size = 40, ring = true }) {
  const ringClasses = ring
    ? 'ring-2 ring-offset-2 ring-offset-background ring-primary'
    : '';
  return (
    <div
      className={`bg-center bg-no-repeat aspect-square bg-cover rounded-full ${ringClasses}`}
      style={{
        backgroundImage: `url("${url}")`,
        width: size,
        height: size,
      }}
      data-alt="User avatar"
      aria-hidden="true"
      role="img"
    />
  );
}

function Progress({ percent }) {
  return (
    <div className="flex flex-col gap-3 pt-4">
      <div className="flex gap-6 justify-between items-center">
        <p className="text-on-surface text-base font-medium">Level 24 XP</p>
        <p className="text-primary text-sm font-semibold">{percent}%</p>
      </div>
      <div className="rounded-full bg-surface-muted h-2.5">
        <div className="h-2.5 rounded-full bg-primary" style={{ width: `${percent}%` }} />
      </div>
      <p className="text-on-surface-muted text-xs font-normal">1250 / 2000 XP to Level 25</p>
    </div>
  );
}

function TopPlayers() {
  const players = [
    {
      rank: 1,
      name: 'ShadowStriker',
      score: 5430,
      colorClass: 'text-primary',
      avatar:
        'https://lh3.googleusercontent.com/aida-public/AB6AXuCQL4K5RqEnPzW8mWNltF2WQedwGf6JNHPqe0phRPzlVxKytd9gj-jsAR7GF3lzIMK3-iNjwPB9xW3GbCXdWJ-QsXZrhZNI7yjksYxgaT8ms_HYTTR_34GCMtxv9zRz27RcCNPEVtwBE0uL3MF2pqtd6RXDInXqi7o3gYRODhe4zJBIltVy7f7BUcWfqkFajb4Cqr0OlNd62QtpxxbkYpGpiUXrRmSAbBXvoxD5yTI6e5oR8yDEkAqrkM0we2yxGT24Vr5Kokeedi0',
      trend: 'up',
    },
    {
      rank: 2,
      name: 'PixelWraith',
      score: 5110,
      colorClass: 'text-slate-300',
      avatar:
        'https://lh3.googleusercontent.com/aida-public/AB6AXuB-KJvL8-JXtJuA69HopeeLX2E3ROTfTTz9te_WLNyAJfhdgqzgs-D9hEzHLcz5KbPDQLaFqNljHCF4sfM0lQpP3RzmO5sSN0eafgb-cBAFkvtCBrgbhYC35WVybzFG3xTyummYHGg4yhO7YjDw7ip_mqXPTa9GE5u5WN9S_m-eblA_QowoVjlbZiDtbxhim59b7MMr3IR5KIL7UAwXja0Jl8lQ-6OSTF8QA530pajfI-iPnxPVE_OnxlEngv_fr6nAxbUavF6NE_0',
      trend: 'down',
    },
    {
      rank: 3,
      name: 'DataDuchess',
      score: 4980,
      colorClass: 'text-amber-600',
      avatar:
        'https://lh3.googleusercontent.com/aida-public/AB6AXuCSyFgTu_b_qr1hrEOn-OdXpdF3FzMsKiR9uAZGtuXjf-D4lzO5mwDvbLcW5QDUvYvEKt5DiQ1wULDD8GIpa6JN-e2rU20UXo5_F0EY_O38hOB1WOVvk67Nms4D09mvHohmhngYiW4IX1MMJc8x-yI_0XHp60g15TH10oNYKulruFT7_CeE_m9SrW_SLxrJadmyP8yJbqCYibn4wKDWqBUaChsXG_yiArovTHfiOBxXysfNhj8BoLolqqA0i6iYKeaguZEgIDUYJY8',
      trend: 'up',
    },
  ];
  return (
    <div className="bg-surface rounded-xl p-6 flex flex-col gap-4">
      <h3 className="text-xl font-bold text-on-surface">Top Players</h3>
      <ul className="space-y-3">
        {players.map((p) => (
          <li key={p.rank} className="flex items-center gap-4">
            <span className={`font-bold text-lg ${p.colorClass}`}>{p.rank}</span>
            <div
              className="bg-center bg-no-repeat aspect-square bg-cover rounded-full size-10"
              style={{ backgroundImage: `url("${p.avatar}")` }}
            />
            <div className="flex-grow">
              <p className="font-medium">{p.name}</p>
              <p className="text-xs text-on-surface-muted">{p.score.toLocaleString()} Score</p>
            </div>
            <span
              className={`material-symbols-outlined ${
                p.trend === 'up' ? 'text-green-400' : 'text-red-400'
              }`}
            >
              {p.trend === 'up' ? 'arrow_drop_up' : 'arrow_drop_down'}
            </span>
          </li>
        ))}
      </ul>
    </div>
  );
}

function LiveFeed({ events, onSend }) {
  const [msg, setMsg] = useState('');
  return (
    <div className="bg-surface rounded-xl p-6 flex flex-col gap-4 flex-grow">
      <h3 className="text-xl font-bold text-on-surface">Live Feed</h3>
      <div className="h-64 overflow-y-auto space-y-3 pr-2">
        {events.map((e, i) => (
          <p key={i} className="text-sm text-on-surface-muted">
            <div dangerouslySetInnerHTML={{ __html: e }} ></div>
          </p>
        ))}
      </div>
      <div className="mt-auto flex gap-2">
        <input
          className="w-full bg-surface-muted border border-white/10 rounded-lg h-10 px-3 text-sm focus:ring-primary focus:border-primary placeholder:text-on-surface-muted"
          placeholder="Say something..."
          type="text"
          value={msg}
          onChange={(e) => setMsg(e.target.value)}
        />
        <button
          onClick={() => {
            if (!msg.trim()) return;
            onSend(msg.trim());
            setMsg('');
          }}
          className="flex items-center justify-center rounded-lg h-10 w-10 bg-primary text-black hover:bg-primary-light transition-all duration-300"
        >
          <span className="material-symbols-outlined text-xl">send</span>
        </button>
      </div>
    </div>
  );
}

function HealthBar({ hp }) {
  const pct = Math.max(0, Math.min(100, hp));
  return (
    <div className="w-full">
      <div className="flex justify-between items-center mb-1">
        <p className="text-sm text-on-surface-muted">Health</p>
        <p className="text-sm text-primary font-semibold">{pct}%</p>
      </div>
      <div className="h-2.5 bg-surface-muted rounded-full">
        <div
          className={`h-2.5 rounded-full ${pct > 50 ? 'bg-green-500' : pct > 25 ? 'bg-amber-400' : 'bg-red-500'}`}
          style={{ width: `${pct}%` }}
        />
      </div>
    </div>
  );
}

function BattleArena({ open, onClose, spectate = false, game }) {
  const [players, setPlayers] = useState([
    {
      name: 'CyberDiceKing',
      level: 24,
      xpPercent: 65,
      avatar:
        'https://lh3.googleusercontent.com/aida-public/AB6AXuBXFCRwa76rM4TXGRYLctBP1SeHROVGDtDm6_G1iLFD4L-nPFW2A0mivS4DuMCBeW4ZuBs1p0gYkIn6uiaywnRWWHlJe-yAJsTt20VPxHqNfaVz7o9NNpXLmTGkglIGE2FBNNWWs1X1fsFDZOrR516seHfruuUsz3StY_vS7wcyGNykm9s2o91jf85-SLwnJlR936IFsEE9ALsW2sL5C3SssGkiO08D2ee2U2rwe-n8DveqLSGq7HNIOq5BNs-Fa_FxLlofAK0ZHdE',
      hp: 100,
      min: 1,
      max: 6,
      lastRoll: null,
      combos: 0,
    },
    {
      name: 'PixelWraith',
      level: 23,
      xpPercent: 40,
      avatar:
        'https://lh3.googleusercontent.com/aida-public/AB6AXuB-KJvL8-JXtJuA69HopeeLX2E3ROTfTTz9te_WLNyAJfhdgqzgs-D9hEzHLcz5KbPDQLaFqNljHCF4sfM0lQpP3RzmO5sSN0eafgb-cBAFkvtCBrgbhYC35WVybzFG3xTyummYHGg4yhO7YjDw7ip_mqXPTa9GE5u5WN9S_m-eblA_QowoVjlbZiDtbxhim59b7MMr3IR5KIL7UAwXja0Jl8lQ-6OSTF8QA530pajfI-iPnxPVE_OnxlEngv_fr6nAxbUavF6NE_0',
      hp: 100,
      min: 1,
      max: 6,
      lastRoll: null,
      combos: 0,
    },
  ]);
  const [active, setActive] = useState(0);
  const [log, setLog] = useState([]);
  const [rolling, setRolling] = useState(false);

  const biasedRoll = (p) => {
    const gamma = 1 / (1 + p.level / 20);
    const r = Math.random() ** gamma;
    const base = Math.floor(p.min + (p.max - p.min) * r);
    return Math.max(p.min, Math.min(p.max, base));
  };

  const strike = () => {
    if (spectate || rolling) return;
    const attacker = active;
    const defender = attacker === 0 ? 1 : 0;

    setRolling(true);
    setTimeout(() => {
      setPlayers((curr) => {
        const pA = { ...curr[attacker] };
        const pB = { ...curr[defender] };
        let dmg = biasedRoll(pA);

        if (pA.lastRoll !== null && pA.lastRoll === dmg) {
          dmg += 2;
          pA.combos += 1;
        }

        pA.lastRoll = dmg;
        pB.hp = Math.max(0, pB.hp - dmg);

        const next = attacker === 0 ? 1 : 0;
        setActive(next);

        const event = `<span class="text-primary font-semibold">${pA.name}</span> rolled <span class="text-on-surface font-semibold">${dmg}</span> damage on <span class="text-primary">${pB.name}</span>${dmg > pA.max ? ' <span class="text-amber-400">(combo)</span>' : ''}.`;
        setLog((l) => [`${event}`, ...l].slice(0, 30));

        return attacker === 0 ? [pA, pB] : [pB, pA];
      });
      setRolling(false);
    }, 350);

    if (!spectate) {
      setTimeout(() => {
        autoStrike();
      }, 800);
    }
  };

  const autoStrike = () => {
    if (rolling) return;
    const attacker = active;
    const defender = attacker === 0 ? 1 : 0;

    setRolling(true);
    setTimeout(() => {
      setPlayers((curr) => {
        const pA = { ...curr[attacker] };
        const pB = { ...curr[defender] };
        let dmg = biasedRoll(pA);

        if (pA.lastRoll !== null && pA.lastRoll === dmg) {
          dmg += 2;
          pA.combos += 1;
        }

        pA.lastRoll = dmg;
        pB.hp = Math.max(0, pB.hp - dmg);

        const next = attacker === 0 ? 1 : 0;
        setActive(next);

        const event = `<span class="text-primary font-semibold">${pA.name}</span> rolled <span class="text-on-surface font-semibold">${dmg}</span> damage on <span class="text-primary">${pB.name}</span>${dmg > pA.max ? ' <span class="text-amber-400">(combo)</span>' : ''}.`;
        setLog((l) => [`${event}`, ...l].slice(0, 30));

        return attacker === 0 ? [pA, pB] : [pB, pA];
      });
      setRolling(false);
    }, 350);
  };

  useEffect(() => {
    if (!open) return;
    const winner = players.find((p) => p.hp <= 0);
    if (winner) return;
    if (spectate) {
      const id = setInterval(() => {
        autoStrike();
      }, 1200);
      return () => clearInterval(id);
    }
  }, [open, spectate]); // eslint-disable-line

  useEffect(() => {
    const winnerIdx = players.findIndex((p) => p.hp <= 0);
    if (winnerIdx !== -1) {
      const loserIdx = winnerIdx === 0 ? 1 : 0;
      const winner = players[loserIdx];
      const event = `<span class="text-primary font-semibold">${winner.name}</span> won the <span class="text-on-surface font-semibold">${game?.title || 'Duel'}</span>.`;
      setLog((l) => [`${event}`, ...l].slice(0, 30));
    }
  }, [players, game]);

  if (!open) return null;

  return (
    <div className="fixed inset-0 z-50 bg-black/70 backdrop-blur-sm flex items-center justify-center p-4">
      <div className="bg-surface rounded-xl border border-white/10 w-full max-w-4xl shadow-xl overflow-hidden">
        <div className="flex items-center justify-between border-b border-white/10 px-6 py-4">
          <div className="flex items-center gap-3">
            <DiceIcon className="size-6 text-primary" />
            <h2 className="text-lg font-bold">{game?.title || 'Duel Arena'}</h2>
            <span className="text-on-surface-muted text-sm">Real-time</span>
          </div>
          <button
            onClick={onClose}
            className="flex items-center justify-center rounded-lg h-9 px-3 bg-surface-muted text-on-surface-muted hover:bg-white/10"
          >
            <span className="material-symbols-outlined">close</span>
          </button>
        </div>
        <div className="grid grid-cols-1 md:grid-cols-3 gap-6 p-6">
          <div className="flex flex-col gap-3 bg-surface-muted rounded-xl p-4">
            <div className="flex items-center gap-3">
              <div
                className="bg-center bg-no-repeat aspect-square bg-cover rounded-full size-12 ring-2 ring-offset-2 ring-offset-surface ring-primary"
                style={{
                  backgroundImage: `url("${players[0].avatar}")`,
                }}
              />
              <div>
                <p className="font-semibold">{players[0].name}</p>
                <p className="text-xs text-primary">Level {players[0].level}</p>
              </div>
            </div>
            <HealthBar hp={players[0].hp} />
          </div>
          <div className="flex flex-col gap-4 items-center justify-center">
            <div className="relative flex items-center justify-center w-40 h-40 rounded-xl bg-surface-muted border border-white/10">
              <div className="absolute inset-0 animate-ping rounded-xl bg-primary/10" />
              <div className="text-6xl font-extrabold text-primary drop-shadow">
                {players[active]?.lastRoll ?? '—'}
              </div>
            </div>
            <button
              onClick={strike}
              disabled={spectate || players.some((p) => p.hp <= 0)}
              className={`flex items-center justify-center rounded-lg h-12 px-5 w-full max-w-xs ${
                spectate || players.some((p) => p.hp <= 0)
                  ? 'bg-surface-muted text-on-surface-muted cursor-not-allowed'
                  : 'bg-primary text-black hover:bg-primary-light hover:shadow-glow-primary transition-all duration-300'
              }`}
            >
              <span className="material-symbols-outlined mr-2">sports_mma</span>
              Strike
            </button>
          </div>
          <div className="flex flex-col gap-3 bg-surface-muted rounded-xl p-4">
            <div className="flex items-center gap-3">
              <div
                className="bg-center bg-no-repeat aspect-square bg-cover rounded-full size-12 ring-2 ring-offset-2 ring-offset-surface ring-primary"
                style={{
                  backgroundImage: `url("${players[1].avatar}")`,
                }}
              />
              <div>
                <p className="font-semibold">{players[1].name}</p>
                <p className="text-xs text-primary">Level {players[1].level}</p>
              </div>
            </div>
            <HealthBar hp={players[1].hp} />
          </div>
          <div className="md:col-span-3">
            <div className="bg-surface-muted rounded-xl p-4 h-40 overflow-y-auto space-y-2">
              {log.map((l, i) => (
                <p key={i} className="text-sm text-on-surface-muted" dangerouslySetInnerHTML={{ __html: l }} />
              ))}
            </div>
          </div>
        </div>
      </div>
    </div>
  );
}

function GameCard({ game, onJoin, onSpectate, onBet }) {
  return (
    <div className="bg-surface rounded-xl p-4 flex flex-col sm:flex-row items-center gap-4 border border-transparent hover:border-primary/50 transition-all duration-300 group">
      <div className="flex-grow w-full">
        <div className="flex justify-between items-center">
          <h3 className="text-lg font-bold text-on-surface">{game.title}</h3>
          <div className="flex items-center gap-2">
            {game.status === 'Live' ? (
              <>
                <span className="relative flex h-2.5 w-2.5">
                  <span className="animate-ping absolute inline-flex h-full w-full rounded-full bg-red-400 opacity-75" />
                  <span className="relative inline-flex rounded-full h-2.5 w-2.5 bg-red-500" />
                </span>
                <span className="text-red-400 text-xs font-bold uppercase tracking-wider">Live</span>
              </>
            ) : (
              <>
                <span className="relative flex h-2.5 w-2.5">
                  <span className="relative inline-flex rounded-full h-2.5 w-2.5 bg-green-500" />
                </span>
                <span className="text-green-400 text-xs font-bold uppercase tracking-wider">Starting Soon</span>
              </>
            )}
          </div>
        </div>
        <div className="flex items-center gap-6 mt-3 text-sm text-on-surface-muted">
          <div className="flex items-center gap-2">
            <span className="material-symbols-outlined text-base">group</span>
            <span>
              {game.players}/{game.capacity} Players
            </span>
          </div>
          <div className="flex items-center gap-2">
            <span className="material-symbols-outlined text-base">monetization_on</span>
            <span>Entry: {game.entry} Tokens</span>
          </div>
          <div className="flex items-center gap-2">
            <span className="material-symbols-outlined text-base">emoji_events</span>
            <span>Prize: {game.prize} Tokens</span>
          </div>
        </div>
      </div>
      <div className="flex flex-row sm:flex-col gap-2 w-full sm:w-auto">
        <button
          onClick={() => onJoin(game)}
          className="w-full sm:w-auto flex min-w-[84px] items-center justify-center rounded-lg h-10 px-4 bg-primary text-black text-sm font-bold tracking-[-0.01em] transition-all duration-300 group-hover:bg-primary-light"
        >
          Join Game
        </button>
        {game.status === 'Live' ? (
          <button
            onClick={() => onSpectate(game)}
            className="w-full sm:w-auto flex min-w-[84px] items-center justify-center rounded-lg h-10 px-4 bg-surface-muted text-on-surface-muted text-sm font-medium hover:bg-white/10"
          >
            Spectate
          </button>
        ) : (
          <button
            onClick={() => onBet(game)}
            className="w-full sm:w-auto flex min-w-[84px] items-center justify-center rounded-lg h-10 px-4 bg-surface-muted text-on-surface-muted text-sm font-medium hover:bg-white/10"
          >
            Bet
          </button>
        )}
      </div>
    </div>
  );
}

function App() {
  const [arenaOpen, setArenaOpen] = useState(false);
  const [spectate, setSpectate] = useState(false);
  const [activeGame, setActiveGame] = useState(null);
  const [feed, setFeed] = useState([
    '<span class="text-primary font-semibold">PixelWraith</span> rolled a critical 6 to win the Neon Duel!',
    '<span class="text-sm text-on-surface-muted"><span class="text-primary font-semibold">New Bet:</span> 200 Tokens on <span class="text-on-surface font-semibold">ShadowStriker</span> by <span class="text-primary">User_77</span>.</span>',
    '<span class="text-primary font-semibold">CyberDiceKing</span> just joined the Cyberpunk Showdown.',
    '<span class="text-primary font-semibold">DataDuchess</span> upgraded their NFT Dice to Level 5.',
    '<span class="text-on-surface font-semibold">Server Message:</span> High Stakes tournament starts in 10 minutes!',
    '<span class="text-primary font-semibold">GlitchGamer</span> was eliminated from Arcade Classic.',
  ]);

  const games = useMemo(
    () => [
      { id: 1, title: 'Cyberpunk Showdown', status: 'Live', players: 6, capacity: 8, entry: 500, prize: 3500 },
      { id: 2, title: 'Neon Duel', status: 'Soon', players: 1, capacity: 2, entry: 1000, prize: 1800 },
      { id: 3, title: 'Arcade Classic', status: 'Full', players: 4, capacity: 4, entry: 100, prize: 350, full: true },
    ],
    []
  );

  useEffect(() => {
    const id = setInterval(() => {
      const messages = [
        '<span class="text-primary font-semibold">Server:</span> New regional match created.',
        '<span class="text-primary font-semibold">Bet:</span> 50 Tokens on <span class="text-on-surface font-semibold">DataDuchess</span>.',
        '<span class="text-primary font-semibold">Upgrade:</span> <span class="text-on-surface font-semibold">ShadowStriker</span> boosted Damage to +2.',
        '<span class="text-primary font-semibold">Match:</span> <span class="text-on-surface font-semibold">Neon Duel</span> starts in 5 minutes.',
      ];
      setFeed((f) => [messages[Math.floor(Math.random() * messages.length)], ...f].slice(0, 50));
    }, 5000);
    return () => clearInterval(id);
  }, []);

  const onJoin = (game) => {
    setActiveGame(game);
    setSpectate(false);
    setArenaOpen(true);
    setFeed((f) => [`<span class="text-primary font-semibold">You</span> joined <span class="text-on-surface font-semibold">${game.title}</span>.`, ...f]);
  };

  const onSpectate = (game) => {
    setActiveGame(game);
    setSpectate(true);
    setArenaOpen(true);
    setFeed((f) => [`<span class="text-primary font-semibold">Spectating</span> <span class="text-on-surface font-semibold">${game.title}</span>.`, ...f]);
  };

  const onBet = (game) => {
    const amount = Math.round(Math.random() * 500 + 50);
    setFeed((f) => [
      `<span class="text-primary font-semibold">New Bet:</span> ${amount} Tokens on <span class="text-on-surface font-semibold">${game.title}</span> by <span class="text-primary">User_${Math.floor(
        Math.random() * 100
      )}</span>`,
      ...f,
    ]);
  };

  const  initLinera = async () => {
    await linera.default();
    const faucet = await new linera.Faucet(
      'https://faucet.testnet-conway.linera.net',
    );
    const wallet = await faucet.createWallet();
    const client = await new linera.Client(wallet);
    document.getElementById('chain-id').innerText = await faucet.claimChain(client);
  }

 

  return (
    <div className="relative flex h-auto min-h-screen w-full flex-col">
      <header className="flex items-center justify-between whitespace-nowrap border-b border-solid border-white/10 px-6 sm:px-10 py-3 sticky top-0 z-50 bg-background/80 backdrop-blur-md">
        <div className="flex items-center gap-4">
          <div className="size-8 text-primary">
            <DiceIcon className="size-8 text-primary" />
          </div>
          <h2 className="text-on-surface text-lg font-bold tracking-[-0.015em]">Dice Tournament</h2>
        </div>
        <div className="hidden md:flex items-center gap-9">
          <a className="text-on-surface text-sm font-medium hover:text-primary transition-colors" href="#">
            Home
          </a>
          <a className="text-on-surface-muted text-sm font-medium hover:text-primary transition-colors" href="#">
            NFTs
          </a>
          <a className="text-on-surface-muted text-sm font-medium hover:text-primary transition-colors" href="#">
            Leaderboard
          </a>
          <a className="text-on-surface-muted text-sm font-medium hover:text-primary transition-colors" href="#">
            Store
          </a>
        </div>
        <div className="flex items-center gap-4">
          <button className="flex max-w-[480px] cursor-pointer items-center justify-center overflow-hidden rounded-full h-10 w-10 bg-surface text-on-surface-muted hover:bg-white/10 transition-colors">
            <span className="material-symbols-outlined text-lg">notifications</span>
          </button>
          <div
            className="bg-center bg-no-repeat aspect-square bg-cover rounded-full size-10 ring-2 ring-offset-2 ring-offset-background ring-primary"
            style={{
              backgroundImage:
                'url("https://lh3.googleusercontent.com/aida-public/AB6AXuBezFDvOfOllyzJ46Hx5yMKk31m_W6v1UM76FxL-4s9QIRMeKsddJ-MmwtASzm0pQsVDOaVPSnOk9gLKQKsUiVHd_tOpSsDFHvxcmMRp0QXL07ugwRKDiFGUq5XEt0Gf-KYoUXv8qbMEp7ZEVgkcvCog_qTLWEcM4uv1EmtkCT6d_5BWv7VeabIBIC48bpQuGIXVxF6b3ftdDYuDZJee6u3VVoq3J4ZDt4sTBIHE-lGW5QWp00reAQCgR9xuYMc-4JpJ2CNFrbvt6s")',
            }}
          />
        </div>
      </header>

      <main className="flex-grow p-4 sm:px-6 lg:px-8 flex flex-col ">
        <div className="grid grid-cols-1 lg:grid-cols-12 gap-6 lg:gap-8  mx-auto flex-1">
          <aside className="lg:col-span-3 flex flex-col gap-6">
            <div className="flex h-full flex-col justify-between bg-surface p-6 rounded-xl">
              <div className="flex flex-col gap-4">
                <div className="flex items-center gap-4">
                  <div
                    className="bg-center bg-no-repeat aspect-square bg-cover rounded-full size-16 ring-2 ring-offset-2 ring-offset-surface ring-primary"
                    style={{
                      backgroundImage:
                        'url("https://lh3.googleusercontent.com/aida-public/AB6AXuBXFCRwa76rM4TXGRYLctBP1SeHROVGDtDm6_G1iLFD4L-nPFW2A0mivS4DuMCBeW4ZuBs1p0gYkIn6uiaywnRWWHlJe-yAJsTt20VPxHqNfaVz7o9NNpXLmTGkglIGE2FBNNWWs1X1fsFDZOrR516seHfruuUsz3StY_vS7wcyGNykm9s2o91jf85-SLwnJlR936IFsEE9ALsW2sL5C3SssGkiO08D2ee2U2rwe-n8DveqLSGq7HNIOq5BNs-Fa_FxLlofAK0ZHdE")',
                    }}
                  />
                  <div className="flex flex-col">
                    <h1 className="text-on-surface text-xl font-bold leading-normal">CyberDiceKing</h1>
                    <p className="text-primary text-sm font-medium leading-normal">Rank: Diamond</p>
                  </div>
                </div>
                <Progress percent={65} />
                <p className="text-on-surface text-base font-normal pt-2">Wallet: 5,000 Tokens</p>
                <div className="border-t border-white/10 pt-4 mt-2 flex flex-col gap-2">
                  <a className="flex items-center gap-3 px-3 py-2 rounded-lg bg-white/5" href="#">
                    <span className="material-symbols-outlined text-primary text-xl">dashboard</span>
                    <p className="text-on-surface text-sm font-medium">Dashboard</p>
                  </a>
                  <a className="flex items-center gap-3 px-3 py-2 rounded-lg hover:bg-white/5 transition-colors" href="#">
                    <span className="material-symbols-outlined text-on-surface-muted text-xl">collections_bookmark</span>
                    <p className="text-on-surface-muted text-sm font-medium">My NFTs</p>
                  </a>
                  <a className="flex items-center gap-3 px-3 py-2 rounded-lg hover:bg-white/5 transition-colors" href="#">
                    <span className="material-symbols-outlined text-on-surface-muted text-xl">history</span>
                    <p className="text-on-surface-muted text-sm font-medium">Match History</p>
                  </a>
                </div>
              </div>
              <div className="flex flex-1 gap-4 flex-col items-stretch mt-6">
                <button
                  onClick={() => onJoin(games[0])}
                  className="flex min-w-[84px] max-w-[480px] cursor-pointer items-center justify-center overflow-hidden rounded-lg h-12 px-5 bg-primary text-black text-base font-bold tracking-[-0.01em] w-full hover:bg-primary-light transition-all duration-300 hover:shadow-glow-primary"
                >
                  <span className="truncate">Create a Game</span>
                </button>
                <button
                  onClick={() => onJoin(games[1])}
                  className="flex min-w-[84px] max-w-[480px] cursor-pointer items-center justify-center overflow-hidden rounded-lg h-12 px-5 bg-surface-muted border border-white/10 text-on-surface text-base font-bold tracking-[-0.01em] w-full hover:bg-white/10 transition-all duration-300"
                >
                  <span className="truncate">Quick Join</span>
                </button>
              </div>
            </div>
          </aside>

          <main className="lg:col-span-6 flex flex-col gap-6">
            <div className="flex flex-col gap-2">
              <h1 className="text-3xl font-bold text-on-surface">Join the Fight</h1>
              <p className="text-on-surface-muted">Active tournaments waiting for a challenger.</p>
            </div>
            <div className="flex items-center gap-2 border-b border-white/10 pb-1">
              <button className="px-4 py-2 text-sm font-semibold rounded-t-lg border-b-2 border-primary text-primary">All Games</button>
              <button className="px-4 py-2 text-sm font-medium text-on-surface-muted hover:text-on-surface transition-colors">High Stakes</button>
              <button className="px-4 py-2 text-sm font-medium text-on-surface-muted hover:text-on-surface transition-colors">Casual</button>
              <button className="px-4 py-2 text-sm font-medium text-on-surface-muted hover:text-on-surface transition-colors">Regional</button>
            </div>
            <div className="flex flex-col gap-4">
              <GameCard game={games[0]} onJoin={onJoin} onSpectate={onSpectate} onBet={onBet} />
              <GameCard game={games[1]} onJoin={onJoin} onSpectate={onSpectate} onBet={onBet} />
              <div className="bg-surface rounded-xl p-4 flex flex-col sm:flex-row items-center gap-4 border border-transparent hover:border-primary/50 transition-all duration-300 group opacity-60">
                <div className="flex-grow w-full">
                  <div className="flex justify-between items-center">
                    <h3 className="text-lg font-bold text-on-surface">Arcade Classic</h3>
                    <div className="flex items-center gap-2">
                      <span className="text-on-surface-muted text-xs font-bold uppercase tracking-wider">Full</span>
                    </div>
                  </div>
                  <div className="flex items-center gap-6 mt-3 text-sm text-on-surface-muted">
                    <div className="flex items-center gap-2">
                      <span className="material-symbols-outlined text-base">group</span>
                      <span>4/4 Players</span>
                    </div>
                    <div className="flex items-center gap-2">
                      <span className="material-symbols-outlined text-base">monetization_on</span>
                      <span>Entry: 100 Tokens</span>
                    </div>
                    <div className="flex items-center gap-2">
                      <span className="material-symbols-outlined text-base">emoji_events</span>
                      <span>Prize: 350 Tokens</span>
                    </div>
                  </div>
                </div>
                <div className="flex flex-row sm:flex-col gap-2 w-full sm:w-auto">
                  <button className="w-full sm:w-auto flex min-w-[84px] items-center justify-center rounded-lg h-10 px-4 bg-surface-muted text-on-surface-muted text-sm font-bold tracking-[-0.01em] cursor-not-allowed" disabled>
                    Join Game
                  </button>
                  <button className="w-full sm:w-auto flex min-w-[84px] items-center justify-center rounded-lg h-10 px-4 bg-surface-muted text-on-surface-muted text-sm font-medium hover:bg-white/10">
                    Spectate
                  </button>
                </div>
              </div>
            </div>
          </main>

          <aside className="lg:col-span-3 flex flex-col gap-6">
            <TopPlayers />
            <LiveFeed
              events={feed}
              onSend={(m) => setFeed((f) => [`<span class="text-primary font-semibold">You:</span> ${m}`, ...f])}
            />
          </aside>
        </div>
      </main>

      <BattleArena open={arenaOpen} onClose={() => setArenaOpen(false)} spectate={spectate} game={activeGame} />
    </div>
  );
}

export default App;
