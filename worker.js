// Module-worker entry; the instrument, protocol and scheduling are Rust.
import init, { start_worker } from './ansaloop.js';
await init();
await start_worker();
