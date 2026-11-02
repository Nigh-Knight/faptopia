# Product

## Register

product

## Users

**Desktop power users** — Browse on desktop with keyboard navigation (Z/X/C). Multiple sources open as tabs. Power-seekers who want fast, keyboard-driven access to curated media.

**Mobile PWA users** — Browse reels-style on phone. Add to Home Screen for a native-like experience. Touch/swipe navigation. Same content, different interaction model.

Both audiences value speed, privacy, and zero-friction browsing. The interface is a tool for personal consumption — quiet, responsive, and unobtrusive.

## Product Purpose

Faptopia is a CLI tool that generates standalone interactive HTML galleries from Reddit (via RedGifs) and 4chan (/gif/ board) media sources. The entire experience ships as a self-contained HTML file — no server, no framework, no loader. You run one command, open the file, and browse.

Success looks like: zero-config browsing that feels faster and more intimate than the source platforms themselves. The gallery disappears as a UI and becomes a private viewing room.

## Brand Personality

**Raw · Intimate · No-nonsense**

- **Voice:** Quiet, confident, nothing-to-prove. No splash screens, no onboarding tours, no branding flourishes. The interface speaks through function.
- **Emotional tone:** The UI creates a private space — dark, warm-lit, insulated from the outside. It feels like opening a folder of carefully curated clips rather than browsing a platform feed. Personal, not performative.
- **Visual character:** Pure dark background (the room is dark), content-first layout (the media is the only thing on stage), deliberate restraint (no unnecessary chrome, no shiny badges, no gradient text, no glass effects). The .svg icon is a raw single-letter F in orange; the wall is black. That's the commitment level.

## Anti-references

- **Don't be Netflix/Hollywood.** No glossy carousels, no floating play-icons, no hero banners, no "premium" gradient overlays. This isn't a streaming platform trying to impress you with production value.
- **Don't be a dashboard.** No data tables, no stat counters, no button bars, no sidebar, no navigation drawer. This isn't an admin panel or an analytics tool.
- **Don't be OnlyFans / tube-site.** No "trending" badges, no featured-strip, no "creator name + subscriber count" card overlay, no grid of thumbnails with play-button overlays. The gallery is scroll-stopped, single-item-at-a-time, content-first.

## Design Principles

1. **Content is the interface.** Every pixel of chrome is justified. The video fills as much of the available space as possible. Tabs, dots, labels are subordinate — there when needed, gone when not.
2. **Frictionless navigation.** Keyboard (Z/X/C) and tap/swipe should feel fluid, never ask "am I on the right thing?" or "where do I go next?". The model is scroll-driven, not click-driven.
3. **Dark as a context, not a theme.** Black isn't an aesthetic choice — it's the room where the media plays. The viewer is in a dark space with only the content illuminated. The background disappears.
4. **Intimate scale.** The experience is personal consumption. No share buttons, no comment sections, no engagement loops. It's your private collection browser.
5. **Performance is UX.** Standalone HTML, synchronous load, lazy video preload, warm SW cache. The tool ships as one file and opens instantly. Every millisecond of load time is a UX defect.

## Accessibility & Inclusion

- WCAG AA color contrast (body text >= 4.5:1 against backgrounds)
- `prefers-reduced-motion: reduce` — all animations must degrade gracefully to instant/crossfade
- Keyboard-navigable (Z/X/C/P, Ctrl+S for save) with focus-visible outlines
- Mobile: supports assistive touch, 44px touch targets, safe-area-insets for notched devices
- Color-coding (platform accents: Reddit orange, 4chan green) must not be the only differentiator — labels exist alongside colors