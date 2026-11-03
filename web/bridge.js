// What a page hosting the simulation does for first person, shared by
// index.html and engine.html (see src/ui/page.rs). Call it with the canvas,
// and the page's hint about clicking the simulation if it has one, before
// gl.js's load().
//
// A browser captures the mouse only when asked from inside the handler of a
// click, and the simulation sees a click a frame too late to ask. So it
// tells the page, a frame ahead, when the next press or release should
// capture the mouse, and the page asks from its own handler. It also tells
// the page when first person is on: then a mouse the browser lets go (Esc
// does this itself, and the key may never reach the canvas), or will not
// capture, opens the simulation's menu, as Esc there does; so does a
// click away from the simulation, which takes the keys from it.
function motusboltzBridge(simulation, hint) {
	"use strict";

	let captureArmed = false;
	let firstPerson = false;

	miniquad_add_plugin({
		name: "motusboltz_page",
		version: 1,
		register_plugin: (importObject) => {
			importObject.env.motusboltz_arm_capture = (armed) => {
				captureArmed = armed !== 0;
			};
			// The simulation's own request to capture the mouse comes a
			// frame after the click, where a browser refuses it, and a second
			// request while the first is being granted can make the browser
			// refuse that too. So the page asks, from the click, and here only
			// lets the mouse go.
			importObject.env.sapp_set_cursor_grab = (grab) => {
				if (!grab && document.pointerLockElement === simulation) {
					document.exitPointerLock();
				}
			};
			importObject.env.motusboltz_first_person = (on) => {
				firstPerson = on !== 0;
				if (firstPerson && hint) {
					hint.hidden = true;
				}
			};
		},
	});

	// Once per click: a second request while the first is being granted can
	// be refused.
	let lastRequest = -Infinity;
	// Where the page was scrolled when the capture was asked for: some
	// browsers scroll the page while the mouse is captured.
	let scrolled = null;
	const capture = (event) => {
		const fresh = event.timeStamp - lastRequest > 500;
		if (event.isTrusted && captureArmed && fresh
			&& document.pointerLockElement !== simulation) {
			lastRequest = event.timeStamp;
			scrolled = [window.scrollX, window.scrollY];
			simulation.requestPointerLock();
		}
	};
	const keepScroll = () => {
		if (scrolled) {
			window.scrollTo(scrolled[0], scrolled[1]);
		}
	};
	simulation.addEventListener("mousedown", capture);
	simulation.addEventListener("mouseup", capture);

	const openMenu = () => {
		for (const type of ["keydown", "keyup"]) {
			simulation.dispatchEvent(
				new KeyboardEvent(type, { code: "Escape", key: "Escape" }));
		}
	};
	document.addEventListener("pointerlockchange", () => {
		keepScroll();
		if (firstPerson && document.pointerLockElement !== simulation) {
			simulation.focus({ preventScroll: true });
			openMenu();
		}
	});
	window.addEventListener("scroll", () => {
		if (document.pointerLockElement === simulation) {
			keepScroll();
		}
	});
	simulation.addEventListener("blur", () => {
		if (firstPerson) {
			openMenu();
		}
	});
	// A refusal may only be of a second request made while the first was
	// still being granted, so look again a moment later.
	document.addEventListener("pointerlockerror", () => {
		setTimeout(() => {
			if (firstPerson && document.pointerLockElement !== simulation) {
				openMenu();
			}
		}, 300);
	});

	return { firstPerson: () => firstPerson };
}
