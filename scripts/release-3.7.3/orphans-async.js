// orphaned canvases with a getImageData surface, one await per iteration
const { Canvas } = require(process.argv[2]);
(async () => {
  let peak = 0;
  for (let i = 0; i < 500; i++) {
    let c = new Canvas(1024, 1024); c.gpu = false;
    c.getContext("2d").fillRect(0, 0, 10, 10);
    c.getContext("2d").getImageData(0, 0, 1, 1); // 4 MB surface, then orphaned
    await new Promise((res) => setImmediate(res));
    peak = Math.max(peak, process.memoryUsage().rss);
  }
  console.log(`${process.argv[3]}: peak rss ${(peak / 2 ** 20).toFixed(0)} MB`);
})();
