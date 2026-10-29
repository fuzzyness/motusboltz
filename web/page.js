// The page around the engine: the cover that hands the mouse and keys to the
// engine on a click, and the Gibbs free energy calculator. Everything is
// inside a function because gl.js already declares globals such as `canvas`.
(() => {
	"use strict";

	// ---- Engine ----

	// Narrowest frame, in CSS pixels, that fits the engine's left panel,
	// right-hand controls and some of the 3D view between them.
	const ENGINE_MIN_WIDTH = 1000;
	const WASM_PATH = "motusboltz.wasm";

	const figure = document.querySelector(".engine");
	const frame = figure.querySelector(".engine-frame");
	const engineCanvas = document.getElementById("glcanvas");
	const cover = figure.querySelector(".engine-cover");
	const narrow = figure.querySelector(".engine-narrow");

	function setEngaged(engaged) {
		figure.dataset.engaged = String(engaged);
	}

	// A click on the cover hands the wheel and keys to the engine; Esc or a
	// click anywhere else gives them back to the page.
	cover.addEventListener("click", () => {
		setEngaged(true);
		engineCanvas.focus({ preventScroll: true });
	});
	engineCanvas.addEventListener("blur", () => setEngaged(false));
	engineCanvas.addEventListener("keydown", (event) => {
		if (event.key === "Escape") {
			engineCanvas.blur();
		}
	});

	// gl.js focuses the canvas as soon as it loads. Keep the keys with the
	// page until the reader clicks the engine, and undo any scroll that
	// focusing caused when the page was opened at a section link.
	engineCanvas.blur();
	const target = location.hash && document.getElementById(location.hash.slice(1));
	if (target) {
		target.scrollIntoView();
	} else {
		window.scrollTo(0, 0);
	}

	function loadEngine() {
		figure.classList.remove("is-narrow");
		narrow.hidden = true;
		cover.hidden = false;
		load(WASM_PATH);
	}

	if (frame.clientWidth >= ENGINE_MIN_WIDTH) {
		loadEngine();
	} else {
		figure.classList.add("is-narrow");
		cover.hidden = true;
		narrow.hidden = false;
		narrow.querySelector(".engine-load").addEventListener("click", loadEngine, { once: true });
	}

	// ---- Gibbs free energy calculator ----

	const J_PER_KJ = 1000;
	const ROOM_TEMPERATURE = 298;
	const T_MAX = 1500;
	// |ΔG| below this (kJ/mol) reads as zero: equilibrium.
	const EQUILIBRIUM_BAND = 0.005;
	const MINUS = "−";

	// Standard values near 25 °C, per mole of the substance named in `per`.
	// Each phrase completes "... on its own".
	const PRESETS = {
		ice: {
			formula: "H₂O(s) → H₂O(l)",
			per: "per mole of ice",
			dh: 6.01,
			ds: 22.0,
			forward: "ice melts",
			reverse: "liquid water freezes",
		},
		coldpack: {
			formula: "NH₄NO₃(s) → NH₄⁺(aq) + NO₃⁻(aq)",
			per: "per mole of ammonium nitrate",
			dh: 25.7,
			ds: 108.7,
			forward: "ammonium nitrate dissolves and takes in heat",
			reverse: "the salt comes back out of the water",
			note: "That heat comes from the water, which is why the pack feels cold.",
		},
		warmer: {
			formula: "2 Fe(s) + 3/2 O₂(g) → Fe₂O₃(s)",
			per: "per mole of rust (Fe₂O₃)",
			dh: -824.2,
			ds: -275.0,
			forward: "iron rusts and gives off heat",
			reverse: "rust breaks back down into iron and oxygen",
			note: "A hand warmer holds fine iron powder, so it rusts fast enough to feel warm.",
		},
		peroxide: {
			formula: "H₂O₂(l) → H₂O(l) + ½ O₂(g)",
			per: "per mole of hydrogen peroxide",
			dh: -98.0,
			ds: 62.9,
			forward: "hydrogen peroxide breaks down into water and oxygen",
			reverse: "water and oxygen join into hydrogen peroxide",
			note: "Spontaneous does not mean fast: it is slow until a catalyst such as yeast " +
				"speeds it up, as in the elephant toothpaste demo.",
		},
		ozone: {
			formula: "3/2 O₂(g) → O₃(g)",
			per: "per mole of ozone",
			dh: 142.7,
			ds: -68.9,
			forward: "oxygen turns into ozone",
			reverse: "ozone turns back into oxygen",
			note: "Ozone forms only where lightning or ultraviolet light supplies the energy.",
		},
	};

	const calc = document.getElementById("calculator");
	const inputs = {
		dh: [document.getElementById("dh-range"), document.getElementById("dh-number")],
		ds: [document.getElementById("ds-range"), document.getElementById("ds-number")],
		t: [document.getElementById("t-range"), document.getElementById("t-number")],
	};
	const presetButtons = calc.querySelectorAll("[data-preset]");
	const reaction = calc.querySelector(".reaction");
	const resultValue = calc.querySelector(".result-value");
	const verdict = calc.querySelector(".verdict");
	const chart = calc.querySelector(".chart svg");
	const signCells = document.querySelectorAll(".signs [data-case]");

	const state = { dh: 0, ds: 0, t: ROOM_TEMPERATURE, preset: null };

	/** ΔG in kJ/mol at temperature `t`. */
	function gibbs(dh, ds, t) {
		return dh - (t * ds) / J_PER_KJ;
	}

	/** A number with a real minus sign, rounded for reading. */
	function formatNumber(value, digits) {
		const text = Math.abs(value).toFixed(digits);
		return value < 0 && Number(text) !== 0 ? MINUS + text : text;
	}

	function formatGibbs(value) {
		if (Math.abs(value) < EQUILIBRIUM_BAND) {
			return "0.00";
		}
		const digits = Math.abs(value) >= 100 ? 1 : 2;
		return (value > 0 ? "+" : "") + formatNumber(value, digits);
	}

	function formatKelvin(value) {
		return `${Math.round(value).toLocaleString("en-US")} K`;
	}

	function capitalize(text) {
		return text.charAt(0).toUpperCase() + text.slice(1);
	}

	/** The temperature where ΔG changes sign, if ΔH and ΔS share a sign. */
	function crossover(dh, ds) {
		if (ds === 0 || dh === 0 || Math.sign(dh) !== Math.sign(ds)) {
			return null;
		}
		return (dh / ds) * J_PER_KJ;
	}

	function describe() {
		const { dh, ds, t } = state;
		const g = gibbs(dh, ds, t);
		const preset = state.preset && PRESETS[state.preset];
		const forward = preset ? preset.forward : "the change happens";
		const reverse = preset ? preset.reverse : "the reverse change happens";
		const at = `At ${formatKelvin(t)}`;
		const sentences = [];

		if (Math.abs(g) < EQUILIBRIUM_BAND) {
			sentences.push(`${at}, ΔG is zero: ${forward} and ${reverse} at the same rate. ` +
				"This is equilibrium.");
		} else if (g < 0) {
			sentences.push(`${at}, ΔG is negative, so ${forward} on its own.`);
		} else if (preset) {
			sentences.push(`${at}, ΔG is positive, so ${forward} only with outside help. ` +
				`The reverse is what happens on its own: ${reverse}.`);
		} else {
			sentences.push(`${at}, ΔG is positive, so the change does not happen on its own, ` +
				"but its reverse does.");
		}

		const switchAt = crossover(dh, ds);
		if (switchAt !== null) {
			const offChart = switchAt > T_MAX ? " (off this graph)" : "";
			const side = dh < 0 ? "below" : "above";
			sentences.push(`ΔG changes sign at ${formatKelvin(switchAt)}${offChart}, where ΔH = TΔS: ` +
				`it is negative ${side} that temperature.`);
		} else if (dh < 0 && ds > 0) {
			sentences.push("It happens on its own at every temperature.");
		} else if (dh > 0 && ds < 0) {
			sentences.push("It never happens on its own, at any temperature.");
		}
		if (preset && preset.note) {
			sentences.push(preset.note);
		}
		return sentences.join(" ");
	}

	// ---- Chart ----

	const SVG_NS = "http://www.w3.org/2000/svg";
	const CHART = { width: 520, height: 300, left: 52, right: 14, top: 30, bottom: 36 };
	// Rough width of one character of chart text, for placing labels.
	const CHAR_WIDTH = 7;
	const LABEL_GAP = 9;
	const T_TICK_STEP = 300;
	const TARGET_Y_TICKS = 5;

	function element(name, attributes, text) {
		const node = document.createElementNS(SVG_NS, name);
		for (const [key, value] of Object.entries(attributes)) {
			node.setAttribute(key, value);
		}
		if (text !== undefined) {
			node.textContent = text;
		}
		return node;
	}

	/** A round tick step (1, 2 or 5 times a power of ten) for `span`. */
	function niceStep(span) {
		const rough = span / TARGET_Y_TICKS;
		const power = 10 ** Math.floor(Math.log10(rough));
		const fraction = rough / power;
		const nice = fraction <= 1 ? 1 : fraction <= 2 ? 2 : fraction <= 5 ? 5 : 10;
		return nice * power;
	}

	function drawChart() {
		const { dh, ds, t } = state;
		const plotW = CHART.width - CHART.left - CHART.right;
		const plotH = CHART.height - CHART.top - CHART.bottom;
		const gEnd = gibbs(dh, ds, T_MAX);

		// The y range always includes zero, with a little room around the line.
		let low = Math.min(0, dh, gEnd);
		let high = Math.max(0, dh, gEnd);
		const span = Math.max(high - low, 1);
		const step = niceStep(span);
		low = Math.floor((low - span * 0.04) / step) * step;
		high = Math.ceil((high + span * 0.04) / step) * step;

		const x = (kelvin) => CHART.left + (kelvin / T_MAX) * plotW;
		const y = (value) => CHART.top + ((high - value) / (high - low)) * plotH;

		chart.replaceChildren(chart.querySelector("title"));
		const zeroY = y(0);

		chart.append(element("rect", {
			class: "region-yes",
			x: CHART.left,
			y: zeroY,
			width: plotW,
			height: CHART.top + plotH - zeroY,
		}));

		for (let value = low; value <= high + step / 2; value += step) {
			const yy = y(value);
			chart.append(element("line", { class: "tick", x1: CHART.left, x2: CHART.left + plotW, y1: yy, y2: yy }));
			chart.append(element("text", { x: CHART.left - 8, y: yy + 4, "text-anchor": "end" },
				formatNumber(value, step < 1 ? 1 : 0)));
		}
		for (let kelvin = 0; kelvin <= T_MAX; kelvin += T_TICK_STEP) {
			chart.append(element("text", {
				x: x(kelvin),
				y: CHART.top + plotH + 18,
				"text-anchor": kelvin === 0 ? "start" : kelvin === T_MAX ? "end" : "middle",
			}, kelvin.toLocaleString("en-US")));
		}
		chart.append(element("line", {
			class: "axis", x1: CHART.left, x2: CHART.left, y1: CHART.top, y2: CHART.top + plotH,
		}));
		chart.append(element("line", {
			class: "zero", x1: CHART.left, x2: CHART.left + plotW, y1: zeroY, y2: zeroY,
		}));
		chart.append(element("text", { x: CHART.left + plotW, y: CHART.height - 2, "text-anchor": "end" },
			"T (K)"));
		chart.append(element("text", { x: 0, y: 14 }, "ΔG (kJ/mol)"));

		// Name the two sides of the zero line where there is room for it.
		const labelX = CHART.left + plotW - 8;
		if (zeroY - CHART.top > 28) {
			chart.append(element("text", { class: "region-label", x: labelX, y: zeroY - 9, "text-anchor": "end" },
				"Not on its own"));
		}
		if (CHART.top + plotH - zeroY > 28) {
			chart.append(element("text", { class: "region-label", x: labelX, y: zeroY + 20, "text-anchor": "end" },
				"Happens on its own"));
		}

		chart.append(element("line", {
			class: "curve", x1: x(0), y1: y(dh), x2: x(T_MAX), y2: y(gEnd),
		}));

		const nowX = x(t);
		const nowY = y(gibbs(dh, ds, t));
		chart.append(element("line", { class: "now-line", x1: nowX, x2: nowX, y1: CHART.top, y2: CHART.top + plotH }));

		const switchAt = crossover(dh, ds);
		if (switchAt !== null && switchAt <= T_MAX) {
			const crossX = x(switchAt);
			chart.append(element("circle", { class: "cross-dot", cx: crossX, cy: zeroY, r: 5 }));
			chart.append(crossLabel(`ΔG = 0 at ${formatKelvin(switchAt)}`, crossX, zeroY, nowX,
				CHART.left, CHART.left + plotW, CHART.top, CHART.top + plotH));
		}
		chart.append(element("circle", { class: "now-dot", cx: nowX, cy: nowY, r: 6 }));
	}

	/** The crossover label, on whichever side of its dot keeps it inside the
	 * plot and clear of the temperature line, above the zero line if there
	 * is room. */
	function crossLabel(text, crossX, zeroY, nowX, left, right, top, bottom) {
		const width = text.length * CHAR_WIDTH;
		const sides = [
			{ anchor: "start", from: crossX + LABEL_GAP, to: crossX + LABEL_GAP + width },
			{ anchor: "end", from: crossX - LABEL_GAP - width, to: crossX - LABEL_GAP },
		];
		const fits = (side) => side.from >= left && side.to <= right;
		const clear = (side) => nowX < side.from - 4 || nowX > side.to + 4;
		const side = sides.find((s) => fits(s) && clear(s)) || sides.find(fits) || sides[0];
		const above = zeroY - top > 30 || bottom - zeroY < 30;
		return element("text", {
			class: "cross-label",
			x: side.anchor === "start" ? side.from : side.to,
			y: above ? zeroY - 10 : zeroY + 20,
			"text-anchor": side.anchor,
		}, text);
	}

	// Dragging on the graph sets the temperature.
	function temperatureAt(event) {
		const box = chart.getBoundingClientRect();
		const scale = CHART.width / box.width;
		const plotX = (event.clientX - box.left) * scale - CHART.left;
		const plotW = CHART.width - CHART.left - CHART.right;
		return Math.round(Math.min(Math.max(plotX / plotW, 0), 1) * T_MAX);
	}

	chart.addEventListener("pointerdown", (event) => {
		chart.setPointerCapture(event.pointerId);
		setValue("t", temperatureAt(event));
	});
	chart.addEventListener("pointermove", (event) => {
		if (chart.hasPointerCapture(event.pointerId)) {
			setValue("t", temperatureAt(event));
		}
	});

	// ---- Wiring ----

	function render() {
		const { dh, ds, t } = state;
		resultValue.textContent = `${formatGibbs(gibbs(dh, ds, t))} kJ/mol`;
		verdict.textContent = describe();
		const preset = state.preset && PRESETS[state.preset];
		reaction.replaceChildren();
		if (preset) {
			const formula = document.createElement("span");
			formula.className = "formula";
			formula.textContent = preset.formula;
			reaction.append(formula, `${capitalize(preset.per)}.`);
		} else {
			reaction.textContent = "Your own values.";
		}
		const key = dh === 0 || ds === 0 ? "" : `${dh < 0 ? "neg" : "pos"}-${ds < 0 ? "neg" : "pos"}`;
		for (const cell of signCells) {
			cell.dataset.active = String(cell.dataset.case === key);
		}
		drawChart();
	}

	/** Sets one value everywhere it shows, except in `typing`, the box the
	 * reader is typing into, then redraws. */
	function setValue(name, value, typing) {
		const [range, number] = inputs[name];
		const clamped = Math.min(Math.max(value, Number(range.min)), Number(range.max));
		state[name] = clamped;
		for (const input of [range, number]) {
			if (input !== typing) {
				input.value = String(clamped);
			}
		}
		render();
	}

	function choosePreset(name) {
		state.preset = name;
		for (const button of presetButtons) {
			button.setAttribute("aria-pressed", String(button.dataset.preset === name));
		}
		const preset = PRESETS[name];
		state.dh = preset.dh;
		state.ds = preset.ds;
		inputs.dh.forEach((input) => { input.value = String(preset.dh); });
		inputs.ds.forEach((input) => { input.value = String(preset.ds); });
		setValue("t", ROOM_TEMPERATURE);
	}

	for (const button of presetButtons) {
		button.addEventListener("click", () => choosePreset(button.dataset.preset));
	}

	for (const [name, pair] of Object.entries(inputs)) {
		for (const input of pair) {
			input.addEventListener("input", () => {
				const value = Number(input.value);
				if (input.value === "" || !Number.isFinite(value)) {
					return;
				}
				// New ΔH or ΔS values describe a different change.
				if (name !== "t" && state.preset) {
					state.preset = null;
					for (const button of presetButtons) {
						button.setAttribute("aria-pressed", "false");
					}
				}
				setValue(name, value, input);
			});
		}
		// Once the reader leaves a box, show the value actually used.
		pair[1].addEventListener("change", () => setValue(name, state[name]));
	}

	choosePreset("ice");
})();
