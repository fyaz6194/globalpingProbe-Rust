// Minimal Socket.IO mock dispatcher for fair Rust vs Node.js probe benchmarking.
// Mimics just enough of the Globalping API protocol to let a real probe connect,
// stay "ready", and receive one deterministic measurement job — fully isolated
// from the public network so results aren't contaminated by other live jobs.
const { Server } = require('socket.io');

const PORT = process.argv[2] || 4000;
const MEASUREMENT_TYPE = process.argv[3] || 'ping';
const TARGET = process.argv[4] || '1.1.1.1';
const PACKETS = Number(process.argv[5] || 16);

const io = new Server(PORT, { cors: { origin: '*' } });
const probes = io.of('/probes');

probes.on('connection', (socket) => {
  console.log(`[mock] probe connected: ${socket.id}`);
  let dispatched = false;

  const dispatch = () => {
    if (dispatched) return;
    dispatched = true;
    const job = {
      measurementId: 'bench-1',
      testId: 'bench-1-0',
      measurement: {
        type: MEASUREMENT_TYPE,
        target: TARGET,
        packets: PACKETS,
        protocol: 'ICMP',
        port: 80,
        ipVersion: 4,
        inProgressUpdates: false,
      },
    };
    console.log(`[mock] DISPATCH at ${Date.now()}: ${JSON.stringify(job)}`);
    socket.emit('probe:measurement:request', job);
  };

  socket.on('probe:status:update', (s) => {
    console.log(`[mock] status: ${JSON.stringify(s)}`);
    if (s === 'ready') dispatch();
  });
  socket.on('probe:dns:update', () => {});
  socket.on('probe:isIPv4Supported:update', () => {});
  socket.on('probe:isIPv6Supported:update', () => {});
  socket.on('probe:stats:report', (s) => console.log(`[mock] stats: ${JSON.stringify(s)}`));

  socket.on('probe:measurement:ack', () => {
    console.log(`[mock] ACK received at ${Date.now()}`);
  });

  socket.on('probe:measurement:result', (payload) => {
    console.log(`[mock] RESULT received at ${Date.now()}`);
    console.log(`[mock] result: ${JSON.stringify(payload).slice(0, 300)}`);
    console.log('DONE');
    setTimeout(() => process.exit(0), 500);
  });

  // Fallback: dispatch anyway after 20s in case the probe never reports "ready"
  // (e.g. Node.js probe's status field/value differs from Rust's).
  setTimeout(dispatch, 20000);
});

console.log(`[mock] listening on :${PORT}, will dispatch ${MEASUREMENT_TYPE} -> ${TARGET} (${PACKETS} packets) 3s after connect`);
