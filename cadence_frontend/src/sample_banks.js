export const CURATED_SAMPLE_BANKS = [
  {
    name: "algorave-dave/samples",
    shortcut: "github:algorave-dave/samples",
    sourceUrl: "https://github.com/algorave-dave/samples",
  },
  {
    name: "AuditeMarlow/samples",
    shortcut: "github:AuditeMarlow/samples",
    sourceUrl: "https://github.com/AuditeMarlow/samples",
  },
  {
    name: "AustinOliverHaskell/ms-teams-sounds-strudel",
    shortcut: "github:AustinOliverHaskell/ms-teams-sounds-strudel",
    sourceUrl: "https://github.com/AustinOliverHaskell/ms-teams-sounds-strudel",
  },
  {
    name: "bruveping/RepositorioDesonidosParaExperimentar02",
    shortcut: "github:bruveping/RepositorioDesonidosParaExperimentar02",
    sourceUrl:
      "https://github.com/bruveping/RepositorioDesonidosParaExperimentar02",
  },
  {
    name: "Bubobubobubobubo/Dough-Amen",
    shortcut: "github:Bubobubobubobubo/Dough-Amen",
    sourceUrl: "https://github.com/Bubobubobubobubo/Dough-Amen",
  },
  {
    name: "Bubobubobubobubo/Dough-Juj",
    shortcut: "github:Bubobubobubobubo/Dough-Juj",
    sourceUrl: "https://github.com/Bubobubobubobubo/Dough-Juj",
  },
  {
    name: "eddyflux/crate",
    shortcut: "github:eddyflux/crate",
    sourceUrl: "https://github.com/eddyflux/crate",
  },
  {
    name: "EloMorelo/samples",
    shortcut: "github:EloMorelo/samples",
    sourceUrl: "https://github.com/EloMorelo/samples",
  },
  {
    name: "emrexdeger/strudelSamples",
    shortcut: "github:emrexdeger/strudelSamples",
    sourceUrl: "https://github.com/emrexdeger/strudelSamples",
  },
  {
    name: "fjpolo/fjpolo-Strudel",
    shortcut: "github:fjpolo/fjpolo-Strudel",
    sourceUrl: "https://github.com/fjpolo/fjpolo-Strudel",
  },
  {
    name: "fstiffo/polifonia-samples",
    shortcut: "github:fstiffo/polifonia-samples",
    sourceUrl: "https://github.com/fstiffo/polifonia-samples",
  },
  {
    name: "hvillase/cavlp-25p",
    shortcut: "github:hvillase/cavlp-25p",
    sourceUrl: "https://github.com/hvillase/cavlp-25p",
  },
  {
    name: "k09/samples",
    shortcut: "github:k09/samples",
    sourceUrl: "https://github.com/k09/samples",
  },
  {
    name: "kaiye10/strudelSamples",
    shortcut: "github:kaiye10/strudelSamples",
    sourceUrl: "https://github.com/kaiye10/strudelSamples",
  },
  {
    name: "mot4i/garden",
    shortcut: "github:mot4i/garden",
    sourceUrl: "https://github.com/mot4i/garden",
  },
  {
    name: "mysinglelise/msl-strudel-samples",
    shortcut: "github:mysinglelise/msl-strudel-samples",
    sourceUrl: "https://github.com/mysinglelise/msl-strudel-samples",
  },
  {
    name: "Nikeryms/Samples",
    shortcut: "github:Nikeryms/Samples",
    sourceUrl: "https://github.com/Nikeryms/Samples",
  },
  {
    name: "prismograph/departure",
    shortcut: "github:prismograph/departure",
    sourceUrl: "https://github.com/prismograph/departure",
  },
  {
    name: "QuantumVillage/quantum-music",
    shortcut: "github:QuantumVillage/quantum-music",
    sourceUrl: "https://github.com/QuantumVillage/quantum-music",
  },
  {
    name: "RikyBac15/samples",
    shortcut: "github:RikyBac15/samples",
    sourceUrl: "https://github.com/RikyBac15/samples",
  },
  {
    name: "salsicha/capoeira_strudel",
    shortcut: "github:salsicha/capoeira_strudel",
    sourceUrl: "https://github.com/salsicha/capoeira_strudel",
  },
  {
    name: "sonidosingapura/rochormatic",
    shortcut: "github:sonidosingapura/rochormatic",
    sourceUrl: "https://github.com/sonidosingapura/rochormatic",
  },
  {
    name: "terrorhank/samples",
    shortcut: "github:terrorhank/samples",
    sourceUrl: "https://github.com/terrorhank/samples",
  },
  {
    name: "tesspilot/samples",
    shortcut: "github:tesspilot/samples",
    sourceUrl: "https://github.com/tesspilot/samples",
  },
  {
    name: "tidalcycles/Dirt-Samples",
    shortcut: "github:tidalcycles/Dirt-Samples/master/",
    sourceUrl: "https://github.com/tidalcycles/Dirt-Samples/master/",
  },
  {
    name: "TodePond/samples",
    shortcut: "github:TodePond/samples",
    sourceUrl: "https://github.com/TodePond/samples",
  },
  {
    name: "TristanCacqueray/mirus",
    shortcut: "github:TristanCacqueray/mirus",
    sourceUrl: "https://github.com/TristanCacqueray/mirus",
  },
  {
    name: "Veikkosuhonen/graffathon25-demo",
    shortcut: "github:Veikkosuhonen/graffathon25-demo",
    sourceUrl: "https://github.com/Veikkosuhonen/graffathon25-demo",
  },
  {
    name: "wyan/livecoding-samples",
    shortcut: "github:wyan/livecoding-samples",
    sourceUrl: "https://github.com/wyan/livecoding-samples",
  },
  {
    name: "yaxu/clean-breaks",
    shortcut: "github:yaxu/clean-breaks",
    sourceUrl: "https://github.com/yaxu/clean-breaks",
  },
];

export function sampleBankDefaultLoadId(shortcut, fallback = "sample_bank") {
  const raw = String(shortcut ?? "").trim() || String(fallback ?? "").trim();
  const normalized = raw.startsWith("github:")
    ? raw.slice("github:".length)
    : raw;
  const slug = normalized
    .toLowerCase()
    .replace(/[^a-z0-9]+/g, "_")
    .replace(/^_+|_+$/g, "");
  return slug || fallback;
}
