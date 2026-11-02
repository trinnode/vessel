const vessels = [
  { id: "000011", value: "$8,412", assets: "BTC · USDC · SOL · ETH", status: "LISTED", price: "$9,000", accent: "from-cyan-400/30" },
  { id: "000008", value: "$2,100", assets: "USDC · NEAR", status: "VAULT", price: null, accent: "from-indigo-400/30" },
  { id: "000003", value: "$14,800", assets: "ETH · BTC · SOL · USDC", status: "VAULT", price: null, accent: "from-fuchsia-400/30" },
];

export default function Home() {
  return <main className="grid-bg min-h-screen">
    <nav className="mx-auto flex max-w-7xl items-center justify-between px-6 py-6">
      <a href="/" className="flex items-center gap-3"><img src="/logo-icon.svg" alt="" className="h-9 w-9" /><span className="mono text-sm tracking-[.35em]">VESSEL</span></a>
      <div className="hidden gap-8 text-sm text-slate-300 md:flex"><a href="#flow">How it works</a><a href="/market">Marketplace</a><a href="/security">Security</a></div>
      <button className="rounded-full border border-cyan-400/50 px-4 py-2 text-sm text-cyan-300 hover:bg-cyan-400/10">Connect wallet</button>
    </nav>
    <section className="mx-auto max-w-7xl px-6 pb-24 pt-20 lg:pt-28">
      <div className="max-w-4xl">
        <p className="mono mb-6 text-xs uppercase tracking-[.35em] text-cyan-300">NEAR / PORTFOLIO OWNERSHIP PROTOCOL</p>
        <h1 className="max-w-4xl text-5xl font-medium leading-[.98] tracking-[-.05em] text-white sm:text-7xl">Pack a wallet.<br /><span className="text-cyan-300">Sell it like an NFT.</span></h1>
        <p className="mt-8 max-w-2xl text-lg leading-8 text-slate-300">Vessel turns a multi-chain portfolio into one programmable NEAR NFT. Deposit from any supported chain, escrow it for sale, and transfer control with a single atomic purchase.</p>
        <div className="mt-10 flex flex-wrap gap-4"><a href="#gallery" className="rounded-full bg-cyan-300 px-6 py-3 font-medium text-ink">Explore vessels</a><a href="#flow" className="rounded-full border border-line px-6 py-3 text-slate-200">See the flow ↓</a></div>
      </div>
      <div className="mt-24 grid gap-5 md:grid-cols-3" id="gallery">{vessels.map((v) => <article key={v.id} className={`rounded-3xl border border-line bg-gradient-to-br ${v.accent} to-panel p-6 shadow-2xl shadow-cyan-950/20`}>
        <div className="flex items-start justify-between"><span className="mono text-xs text-slate-400">VESSEL / #{v.id}</span><span className={`rounded-full px-3 py-1 text-[10px] tracking-widest ${v.status === "LISTED" ? "bg-cyan-300 text-ink" : "bg-white/10 text-cyan-200"}`}>{v.status}</span></div>
        <div className="mt-16"><p className="mono text-4xl text-white">{v.value}</p><p className="mt-3 text-sm text-slate-300">{v.assets}</p></div>
        <div className="mt-8 flex items-center justify-between border-t border-line pt-4 text-sm">{v.price ? <><span className="text-slate-400">Ask {v.price}</span><a href="/market" className="text-cyan-300">View listing →</a></> : <span className="text-slate-400">Withdrawals enabled</span>}</div>
      </article>)}</div>
    </section>
    <section id="flow" className="mx-auto max-w-7xl border-t border-line px-6 py-24">
      <p className="mono text-xs tracking-[.3em] text-cyan-300">THE OWNERSHIP LOOP</p><h2 className="mt-4 max-w-2xl text-4xl tracking-[-.04em] text-white">One NFT. One vault. Every chain.</h2>
      <div className="mt-12 grid gap-4 md:grid-cols-4">{[
        ["01", "Mint", "A NEAR NFT creates a dedicated sub-account wallet."],
        ["02", "Pack", "Deposit BTC, ETH, SOL, stablecoins through Intents."],
        ["03", "Escrow", "Listing freezes withdrawals before the NFT leaves you."],
        ["04", "Transfer", "The buyer receives the NFT and full portfolio control."],
      ].map(([n, title, copy]) => <div key={n} className="rounded-2xl border border-line bg-panel p-6"><span className="mono text-sm text-cyan-300">{n}</span><h3 className="mt-12 text-xl text-white">{title}</h3><p className="mt-3 text-sm leading-6 text-slate-400">{copy}</p></div>)}</div>
    </section>
    <footer className="border-t border-line px-6 py-10"><div className="mx-auto flex max-w-7xl flex-col justify-between gap-4 text-sm text-slate-400 md:flex-row"><span className="mono tracking-[.25em] text-slate-200">VESSEL / NEAR</span><span>Non-custodial portfolio ownership. Verify every contract before depositing assets.</span><span><a href="/security" className="text-cyan-300">Security model</a> · <a href="/market" className="text-cyan-300">Market</a></span></div></footer>
  </main>;
}
