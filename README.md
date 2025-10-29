Fast-paced, probabilistic PvP tournaments using verifiable dice-roll combat; wins grant XP that improves damage/health probabilities; orders/positions/upgrades are dynamic NFTs; matches can be staked or free; matchmaking is grade-based and games run on temporary Linera microchains for real‑time play.


High-level gameplay loop
Player deposits collateral / enters match (or plays free).
Match scheduled and a temporary match microchain is created.
Pre-match: players optionally stake tokens and can equip NFTs/upgrades.
Combat: rounds of timed dice-roll exchanges determine damage; combos and modifiers apply.
Post-match: winner receives XP, stake payouts (minus fee), dynamic NFTs update, event logged on chain; leaderboard updates.
Players can sell/transfer upgrade NFTs; predictions (bets) can be placed against matches.

NFT upgrades & dynamics

DynamicUpgradeNFT: stores stat deltas (e.g., +1 base_dice, +2 HP, +1 damage_mod), rarity, usage (permanent vs consumable).
Visuals: metadata points to dynamic art (e.g., animated SVG or off‑chain image with on‑chain version hash).
Upgrade application: applied when equipping; metadata and owner chain updated; upgrades can be burned or unequipped.
Matchmaking: grade-based algorithm
Use an Elo or Glicko-2 rating with initial calibration: new players start at 1500.
Matchmaking buckets: find opponent with rating ±100; expand after timeouts.
For tournaments: seeded by rating.
For staking matches: require both players meet min rating or allow open pools.
Microchain architecture (recommended)
User microchains: custody of tokens, inventory of NFTs, signing operations.
Match microchain (temporary chain per match): handles real-time rounds, receives player messages (commit, reveal, action), computes verified outcomes, publishes events.
Tournament microchain (optional): for bracket management and scheduling.
NFT registry / marketplace app chain: canonical registry for upgrade NFTs and position NFTs.
Oracle microchain: VRF or result resolution & dispute handling.
Typical cross-chain flow (join → play → settle)

Player A posts JoinMatch operation on their user chain with stake and chosen NFT(s).
User chain sends cross-chain message to MatchCoordinator (app chain) to create a match microchain.
Match microchain created and notified to both players.
Players send commit seeds to match chain (via user chains → match chain).
On each round, players reveal seeds (or match chain requests VRF on timeout); match chain computes rolls deterministically, updates MatchState events, and notifies clients.
On match end, match microchain sends payout messages to user chains and NFT update messages to NFT registry chain.
Match microchain can be retired and archived.
Rust-style state structs & message types (sketch)
Below are concise sketches you can drop into a Linera contract (conceptual).


### Core Contracts Design

**1. PlayerRegistry Contract**
- **Player Stats Management**: Store player profiles with XP, level, health points (HP), base damage range, win/loss records
- **Dynamic Attributes**: As players gain XP, their damage range expands (e.g., Level 1: 1-10 damage, Level 5: 5-25 damage)
- **Skill Tiers**: Categorize players into tiers (Bronze, Silver, Gold, Platinum, Diamond) based on cumulative XP for matchmaking

**2. BattleEngine Contract**
- **Real-time Combat Logic**: 
  - Each "strike" button press triggers Linera's randomness to generate damage within player's range
  - Track hit sequences for combo detection (hitting same damage threshold twice = 1.5x multiplier)
  - Turn-based system with time limits (30 seconds per turn)
  - HP depletion determines winner
  
- **Matchmaking Algorithm**:
  - Queue system that pairs players within same tier or ±1 tier
  - ELO-like rating adjustment after each battle
  - Prevents mismatches (no Bronze vs Diamond)

**3. NFT Evolution Contract**
- **Dynamic NFT Metadata**: 
  - NFTs level up with player XP (visual upgrades at levels 5, 10, 20, 50)
  - Stats embedded in NFT: damage multipliers, critical hit chance, defense rating
  - Rarity tiers that unlock special abilities (rare NFTs give 10% HP boost)
  
- **Upgrade Mechanics**:
  - Burn tokens + XP to upgrade NFT tier
  - Merge two NFTs to create higher rarity (with probability of success)

**4. StakingArena Contract**
- **Battle Stakes**:
  - Players stake tokens before battle (winner takes 90%, 10% to treasury)
  - Free-play mode with no stakes but reduced XP gains (50% of staked battles)
  
- **Spectator Betting**:
  - Users bet on live battles with real-time odds
  - Odds calculated based on player stats, tier difference, recent win rates
  - Betting pool distributed among winners proportionally

**5. Tournament Contract**
- **Solo Tournaments**:
  - Bracket-style elimination (8, 16, 32, 64 players)
  - Entry fees pooled for prize distribution (1st: 50%, 2nd: 30%, 3rd-4th: 10% each)
  
- **Team Tournaments**:
  - Teams of 3-5 players
  - Team XP = average of all members' XP
  - Team battles: sequential 1v1s or all-vs-all damage accumulation mode
  - Loot/rewards distributed to team wallet, members vote on distribution

**6. SchedulingOracle Contract**
- **Scheduled Battles**:
  - Players challenge each other with specific time slots
  - Escrow stakes until battle starts
  - No-show = automatic forfeit + penalty
  
- **Prediction Markets**:
  - Open prediction pools for scheduled high-profile matches
  - Lock predictions 5 minutes before battle
  - Payout based on AMM-style odds calculation

### Key Probability & Randomness Mechanics

**Damage Calculation Formula**:
```
Base Damage = Random(MinDamage, MaxDamage) // From Linera randomness
NFT Multiplier = 1.0 + (NFT.damageBonus / 100)
Combo Multiplier = IsCombo ? 1.5 : 1.0
Critical Hit = Random(1, 100) <= CritChance ? 2.0 : 1.0

Final Damage = Base × NFT Multiplier × Combo Multiplier × Critical Hit
```

**XP Progression System**:
- Win: +100 XP (base) × (1 + OpponentTierDifference × 0.3)
- Loss: +25 XP (participation)
- Combo hits: +15 XP per combo
- Critical hits: +10 XP per critical

**Level-Up Thresholds**:
- Level = floor(sqrt(TotalXP / 100))
- This creates smooth but slowing progression (Level 10 needs 10,000 XP, Level 20 needs 40,000 XP)

### Smart Contract Flow Example

**Solo Battle Flow**:
1. Player A clicks "Find Match" → enters queue with tier + stake amount
2. Player B matches → both confirm → stake escrowed
3. Battle starts → each player takes turns clicking "Strike"
4. Each strike triggers:
   - Request random value from Linera
   - Calculate damage using formula above
   - Subtract from opponent's HP
   - Check for combo conditions
   - Emit event for UI updates
5. HP reaches 0 → declare winner → distribute stakes → award XP → update NFT

**Team Tournament Flow**:
1. Team registration (3-5 players) with entry fee
2. Team matchmaking based on average team XP
3. Battle format: each team member fights opponent once (best 3-of-5)
4. Team with most individual wins advances
5. Winning team splits prize according to predefined shares or voting

✅ Real-time battles with verifiable randomness  
✅ Progressive XP system with meaningful NFT evolution  
✅ Fair matchmaking preventing seal-clubbing  
✅ Multiple revenue streams (stakes, tournaments, betting)  
✅ Spectator engagement through live betting  
✅ Team dynamics for social gameplay  
✅ Scheduled battles for competitive players  

