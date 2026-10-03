// Rogueball's bridge to the browser: saves in localStorage, a console log, and the Gamepad API.
// Registered as a miniquad plugin, so these become wasm imports under "env".
"use strict";
(function () {
    const enc = new TextEncoder();
    const dec = new TextDecoder();
    const bytes = (ptr, len) => new Uint8Array(wasm_memory.buffer, ptr, len);
    const str = (ptr, len) => dec.decode(bytes(ptr, len).slice());
    const KEY = (k) => "rogueball:" + k;

    function pad() {
        const pads = navigator.getGamepads ? navigator.getGamepads() : [];
        for (const p of pads) if (p && p.connected) return p;
        return null;
    }

    miniquad_add_plugin({
        name: "rogueball",
        version: 1,
        register_plugin: function (importObject) {
            const env = importObject.env;
            env.rb_log = (ptr, len) => console.log("[rogueball] " + str(ptr, len));
            env.rb_store_set = (kp, kl, vp, vl) => {
                try { localStorage.setItem(KEY(str(kp, kl)), str(vp, vl)); } catch (e) { console.warn(e); }
            };
            env.rb_store_len = (kp, kl) => {
                let v = null;
                try { v = localStorage.getItem(KEY(str(kp, kl))); } catch (e) { }
                return v === null ? -1 : enc.encode(v).length;
            };
            env.rb_store_get = (kp, kl, out) => {
                let v = "";
                try { v = localStorage.getItem(KEY(str(kp, kl))) || ""; } catch (e) { }
                const b = enc.encode(v);
                bytes(out, b.length).set(b);
            };
            env.rb_pad_buttons = () => {
                const p = pad();
                if (!p) return -1;
                let bits = 0;
                for (let i = 0; i < Math.min(p.buttons.length, 16); i++) {
                    const b = p.buttons[i];
                    if (b.pressed || b.value > 0.3) bits |= 1 << i;
                }
                return bits;
            };
            env.rb_pad_axis = (i) => {
                const p = pad();
                return p && i < p.axes.length ? p.axes[i] : 0;
            };
        },
    });
})();
