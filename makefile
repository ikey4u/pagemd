.PHONY: build dev css static

build:
	@brosion build
	@$(MAKE) css
	@$(MAKE) static

dev:
	@brosion dev &
	@npm run build:css:watch

css:
	@npm run build:css

# Copy all static files that brosion doesn't manage
static:
	@# --- debug ---
	@cp src/sidepanel/sidepanel.html dist/debug/sidepanel/
	@cp src/sidepanel/sidepanel.css dist/debug/sidepanel/
	@cp src/options/options.html dist/debug/options/
	@cp assets/icons/icon-32.png dist/debug/sidepanel/icon.png
	@cp manifests/chrome/manifest.json dist/debug/manifest.json
	@# --- release ---
	@cp src/sidepanel/sidepanel.html dist/release/chrome/sidepanel/
	@cp src/sidepanel/sidepanel.css dist/release/chrome/sidepanel/
	@cp src/options/options.html dist/release/chrome/options/
	@cp dist/debug/sidepanel/styles.css dist/release/chrome/sidepanel/
	@cp dist/debug/sidepanel/styles.css dist/release/chrome/options/
	@cp assets/icons/icon-32.png dist/release/chrome/sidepanel/icon.png
	@cp manifests/chrome/manifest.json dist/release/chrome/manifest.json
