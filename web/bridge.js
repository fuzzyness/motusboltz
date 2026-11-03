// What a page hosting the simulation does for first person and full
// screen, shared by index.html and engine.html (see src/ui/page.rs). Call it
// with the canvas, and the page's hint about clicking the simulation if it
// has one, before gl.js's load().
//
// A browser captures the mouse, or goes full screen, only when asked from
// inside the handler of a click, and the simulation sees a click a frame
// too late to ask. So it tells the page, a frame ahead, what the next press
// on it should do: capture the mouse, or switch full screen on or off. The
// page asks from its own handler.
//
// The page is the one that knows whether the mouse is captured, so the
// simulation asks it each frame, rather than keeping a guess of its own:
// whether a capture is held, still being granted, or neither, and whether
// it was lost or refused since the simulation last asked, which opens its
// menu. A click away from the simulation, which takes the keys from it,
// counts as losing it too.
//
// Browsers send these events their own ways. Safari, granting a capture,
// shows a banner and may blur the window, and may report a change of the
// capture before it holds it; neither is a capture lost. So only a capture
// that was held and is no longer counts as let go, and a blur counts only
// once the keys have really gone to something else on the page.
//
// Add ?debug to the page's address to log what happens, to the console
// and to a list in the page's corner, which a screenshot can show.
function motusboltzBridge(simulation, hint) {
	"use strict";

	// Lines the list in the corner keeps.
	const DEBUG_LINES = 24;
	const debug = new URLSearchParams(location.search).has("debug");
	const started = performance.now();
	let shown = null;
	if (debug) {
		shown = document.createElement("pre");
		shown.style.cssText = "position:fixed;left:8px;bottom:8px;z-index:9999;"
			+ "margin:0;padding:6px 8px;max-width:46em;white-space:pre-wrap;"
			+ "font:11px/1.35 ui-monospace,monospace;color:#d7ffd7;"
			+ "background:rgba(0,0,0,0.78);border-radius:6px;pointer-events:none";
		document.body.append(shown);
	}
	const log = (...what) => {
		if (!debug) {
			return;
		}
		console.log("[motusboltz]", ...what);
		const at = ((performance.now() - started) / 1000).toFixed(2);
		const locked = document.pointerLockElement;
		const holder = locked ? (locked.id || locked.tagName) : "none";
		const focused = document.activeElement;
		const keys = focused ? (focused.id || focused.tagName) : "none";
		const line = `${at}s ${what.join(" ")}  [lock: ${holder}, keys: ${keys}]`;
		const lines = (shown.textContent ? shown.textContent.split("\n") : []);
		lines.push(line);
		shown.textContent = lines.slice(-DEBUG_LINES).join("\n");
	};

	// What the next press on the simulation does (see page.rs).
	const ARM_NOTHING = 0;
	const ARM_CAPTURE = 1;
	const ARM_FULL_SCREEN = 2;
	// How the capture stands, as the simulation is told (see page.rs).
	const FREE = 0;
	const PENDING = 1;
	const CAPTURED = 2;
	// A capture asked for and neither granted nor refused within this many
	// milliseconds is taken as refused; some browsers never say.
	const PENDING_LIMIT = 1500;

	let armed = ARM_NOTHING;
	let firstPerson = false;
	// When the capture now being granted was asked for, or null.
	let pendingSince = null;
	// The capture was lost or refused since the simulation last asked.
	let lost = false;
	// The capture was held when last seen, so a change now lets it go.
	let held = false;
	// Milliseconds after a blur before it counts, once the window has had
	// time to come back and the keys are seen to have moved on.
	const BLUR_SETTLE = 150;

	const captured = () => document.pointerLockElement === simulation;
	const fullScreen = () => document.fullscreenElement === simulation;
	const loseCapture = (why) => {
		pendingSince = null;
		if (firstPerson) {
			log("capture lost:", why);
			lost = true;
		}
	};

	miniquad_add_plugin({
		name: "motusboltz_page",
		version: 2,
		register_plugin: (importObject) => {
			importObject.env.motusboltz_arm = (what) => {
				if (what !== armed) {
					log("armed:", what);
				}
				armed = what;
			};
			importObject.env.motusboltz_first_person = (on) => {
				firstPerson = on !== 0;
				log("first person:", firstPerson);
				if (!firstPerson) {
					lost = false;
				}
				if (firstPerson && hint) {
					hint.hidden = true;
				}
			};
			importObject.env.motusboltz_capture = () => {
				if (captured()) {
					return CAPTURED;
				}
				if (pendingSince !== null
					&& performance.now() - pendingSince > PENDING_LIMIT) {
					loseCapture("no answer");
				}
				return pendingSince !== null ? PENDING : FREE;
			};
			importObject.env.motusboltz_take_lost = () => {
				const was = lost;
				lost = false;
				if (was) {
					log("the simulation takes the loss");
				}
				return was ? 1 : 0;
			};
			importObject.env.motusboltz_full_screen = () => (fullScreen() ? 1 : 0);
			// Macroquad repeats its own idea of the capture to gl.js every
			// frame, and that idea lags the page's: the frame after a click
			// captured the mouse, it still says free, and would let it go at
			// once. Its request to capture comes a frame late too, where a
			// browser refuses it. So the page ignores it, captures from the
			// click, and lets go only when the simulation asks (see
			// motusboltz_release).
			let grab = null;
			importObject.env.sapp_set_cursor_grab = (wanted) => {
				if (wanted !== grab) {
					grab = wanted;
					log(`Macroquad's capture: ${wanted ? "on" : "off"} (ignored)`);
				}
			};
			importObject.env.motusboltz_release = () => {
				if (captured()) {
					log("the simulation lets the mouse go");
					document.exitPointerLock();
				}
			};
		},
	});

	// Where the page was scrolled when the capture was asked for: some
	// browsers scroll the page while the mouse is captured, or when it is
	// let go.
	let scrolled = null;
	const keepScroll = () => {
		if (scrolled) {
			window.scrollTo(scrolled[0], scrolled[1]);
		}
	};

	const capture = () => {
		if (captured() || pendingSince !== null) {
			return;
		}
		log("asking to capture the mouse");
		pendingSince = performance.now();
		scrolled = [window.scrollX, window.scrollY];
		simulation.focus({ preventScroll: true });
		// Newer browsers answer with a promise, older ones with events.
		const asked = simulation.requestPointerLock();
		if (asked && typeof asked.catch === "function") {
			asked.catch((error) => loseCapture(`refused (${error && error.name})`));
		}
	};

	const switchFullScreen = () => {
		if (fullScreen()) {
			log("leaving full screen");
			document.exitFullscreen();
		} else if (simulation.requestFullscreen) {
			log("going full screen");
			scrolled = [window.scrollX, window.scrollY];
			const asked = simulation.requestFullscreen();
			if (asked && typeof asked.catch === "function") {
				asked.catch((error) => log("full screen refused:", error && error.name));
			}
		}
	};

	// A press on the full-screen button, which switches it once let go:
	// switched mid-click, the canvas can miss the release, and the
	// simulation then takes the button as held for good.
	let fullScreenPressed = false;
	// A press does what the simulation armed it for. The release counts
	// too, for the figure, which is dropped by letting go.
	const press = (event) => {
		if (!event.isTrusted || event.button !== 0) {
			return;
		}
		if (armed !== ARM_NOTHING) {
			log(event.type, "armed:", armed);
		}
		if (armed === ARM_CAPTURE) {
			capture();
		} else if (armed === ARM_FULL_SCREEN && event.type === "mousedown") {
			fullScreenPressed = true;
		} else if (armed === ARM_FULL_SCREEN && fullScreenPressed) {
			switchFullScreen();
		}
		if (event.type === "mouseup") {
			fullScreenPressed = false;
		}
	};
	simulation.addEventListener("mousedown", press);
	simulation.addEventListener("mouseup", press);

	document.addEventListener("pointerlockchange", () => {
		keepScroll();
		if (captured()) {
			log("mouse captured");
			pendingSince = null;
			held = true;
		} else if (held) {
			held = false;
			loseCapture("let go");
		} else {
			log("capture changed, not yet held");
		}
	});
	document.addEventListener("pointerlockerror", () => loseCapture("refused"));
	if (debug) {
		window.addEventListener("blur", () => log("window blur"));
		window.addEventListener("focus", () => log("window focus"));
		document.addEventListener("keydown", (event) => log("key", event.code));
		document.addEventListener("visibilitychange", () => log("visibility", document.visibilityState));
	}
	window.addEventListener("scroll", () => {
		if (captured() || pendingSince !== null) {
			keepScroll();
		}
	});
	simulation.addEventListener("blur", () => {
		setTimeout(() => {
			const away = document.activeElement !== simulation;
			if (firstPerson && away && !captured() && pendingSince === null) {
				loseCapture("clicked away");
			} else {
				log("blur ignored");
			}
		}, BLUR_SETTLE);
	});
	// The canvas takes the whole screen, and back: tell gl.js its size
	// changed once the browser has laid it out again.
	document.addEventListener("fullscreenchange", () => {
		log("full screen:", fullScreen());
		requestAnimationFrame(() => window.dispatchEvent(new Event("resize")));
		if (!fullScreen()) {
			keepScroll();
		}
	});

	return { firstPerson: () => firstPerson };
}
