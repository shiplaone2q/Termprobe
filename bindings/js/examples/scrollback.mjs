import { Termprobe } from "../dist/index.js";

const terminal = Termprobe.ephemeral(undefined, { backend: "alacritty" });
try {
  await terminal.run(process.execPath, ["-e",
    "for(let i=0;i<40;i++)console.log('history-'+i);process.stdin.resume()",
  ], { rows: 5, cols: 40 });
  await terminal.getByText("history-39").expect();
  const full = await terminal.text({ full: true });
