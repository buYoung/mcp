import { KnownBus } from './known';
import { appBus, handleSaved } from './events';
function opaque(bus: KnownBus) { bus.emit('opaque'); }
function local() { const bus = new KnownBus(); bus.emit('local'); }
function shadow({ appBus }: { appBus: unknown }) { appBus.emit('wrong-shadow'); }
appBus.emit(dynamicKey());
appBus.on('unknown-handler', getHandler());
if (isReady()) { appBus.once('conditional', handleSaved); }
appBus.off('conditional', handleSaved);
if (false) { appBus.on('inactive', handleSaved); }
const unrelated = { emit(key: string) {} }; unrelated.emit('wrong-method');
const prose = "appBus.emit('wrong-string')";
// appBus.emit('wrong-comment');
