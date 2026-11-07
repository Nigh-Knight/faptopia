---
name: Faptopia
description: Standalone interactive media galleries from Reddit and 4chan
colors:
  primary-bg: "#000000"
  surface-elevated: "#0a0a0a"
  surface-raised: "#141414"
  ink-primary: "#f0f0f0"
  ink-secondary: "#888888"
  ink-muted: "#555555"
  accent-reddit: "#ff4500"
  accent-4chan: "#2ea44f"
  border-subtle: "rgba(255,255,255,0.06)"
typography:
  label:
    fontFamily: "'Inter', -apple-system, BlinkMacSystemFont, system-ui, sans-serif"
    fontSize: "0.72rem"
    fontWeight: 600
    letterSpacing: "0.04em"
  label-ui:
    fontFamily: "'Inter', -apple-system, BlinkMacSystemFont, system-ui, sans-serif"
    fontSize: "0.8rem"
    fontWeight: 600
    letterSpacing: "0.08em"
  body-small:
    fontFamily: "'Inter', -apple-system, BlinkMacSystemFont, system-ui, sans-serif"
    fontSize: "0.85rem"
    fontWeight: 500
  body-tab:
    fontFamily: "'Inter', -apple-system, BlinkMacSystemFont, system-ui, sans-serif"
    fontSize: "0.85rem"
    fontWeight: 500
  label-chip:
    fontFamily: "'Inter', -apple-system, BlinkMacSystemFont, system-ui, sans-serif"
    fontSize: "0.75rem"
    fontWeight: 500
    letterSpacing: "0.02em"
rounded:
  sm: "4px"
  md: "8px"
  full: "50%"
spacing:
  xs: "4px"
  sm: "8px"
  md: "12px"
  lg: "16px"
  xl: "20px"
components:
  tab-button:
    backgroundColor: "#141414"
    textColor: "#888888"
    rounded: "8px"
    padding: "8px 16px"
  tab-button-active:
    backgroundColor: "#1a1a1a"
    textColor: "#f0f0f0"
    rounded: "8px"
    padding: "8px 16px"
  gallery-toggle-reddit:
    backgroundColor: "#ff4500"
    textColor: "#f0f0f0"
    rounded: "8px"
    padding: "8px 16px"
  gallery-toggle-4chan:
    backgroundColor: "#0f9d58"
    textColor: "#f0f0f0"
    rounded: "8px"
    padding: "8px 16px"
  board-dot:
    backgroundColor: "rgba(255,255,255,0.2)"
    rounded: "50%"
    size: "8px"
  board-dot-active-reddit:
    backgroundColor: "#ff4500"
    rounded: "4px"
    width: "20px"
    height: "8px"
  source-label:
    backgroundColor: "rgba(0,0,0,0.5)"
    textColor: "rgba(255,255,255,0.9)"
    rounded: "16px"
    padding: "5px 12px"
  nav-hint:
    backgroundColor: "rgba(10,10,10,0.85)"
    textColor: "#888888"
    rounded: "12px"
    padding: "10px 20px"
---

# Design System: Faptopia

## 1. Overview

**Creative North Star: "The Dark Room"**

Faptopia's gallery is not a UI — it's a private room. The walls are black, the lights are off, and the only thing illuminated is the content. The interface disappears into the dark; what remains is a scroll of videos with the barest navigation to move through them. Nothing glow, nothing shiny, nothing announces itself.

This system explicitly rejects: streaming-platform gloss (gradient overlays, floating play buttons, hero banners), dashboard sterility (data tables, sidebar nav, stat counters), and tube-site density (thumbnails grids, trending badges, engagement loops). It is a single-purpose tool: open one file, watch content, close the file. No onboarding, no splash, no performance.

**Key Characteristics:**
- Pure black background as the default state of the room, not as an aesthetic choice
- Tonal layering, not shadows: elevation is communicated entirely through background lightness (black → #0a0a0a → #141414)
- Platform identity via two accent colors (Reddit orange / 4chan green), used only where platform context is needed — the rest of the interface is pure grayscale
- Single-column browsing: desktop horizontal full-height scroll-snap; mobile is a horizontal board carousel whose inner content is a Reels-style vertical snap feed — one full-screen video per viewport
- Every pixel of chrome is justified by function. Tabs are minimal pills; the nav hint is a single bar; board dots are 8px circles

## 2. Colors

The palette is a grayscale hierarchy with two platform accent colors. Gray is not neutral-warm or neutral-cool — it is pure gray, deriving from white at different opacities on black.

### Primary

- **Accent Reddit** (`#ff4500`, oklch(0.59 0.18 32)): Reddit platform identity. Used on the gallery toggle when viewing 4chan (the "switch to Reddit" button), the Reddit platform button on mobile, and the active board-dot when viewing Reddit content. Also the source-label dot. Also the keyboard focus-visible outline. Also the autoscroll alert background.
- **Accent 4chan** (`#2ea44f`, oklch(0.62 0.12 150)): 4chan platform identity. Used symmetrically: the "switch to 4chan" gallery toggle, the 4chan mobile platform button, board-dot active state when viewing 4chan content.

### Neutral

- **Room** (`#000000`): The body background. This is the default state of the room; all content sits on top of it.
- **Surface** (`#0a0a0a`): The tab bar, platform switcher, and any persistent chrome that needs to feel present but not illuminated.
- **Elevated** (`#141414`): Tab buttons at rest, the nav hint background, the platform button gradients.
- **Hover** (`#1a1a1a`): Tab buttons on hover, active state background.
- **Ink Primary** (`#f0f0f0`): Body text, active tab text, platform switcher active text.
- **Ink Secondary** (`#888888`): Resting tab text, nav hint text, progress counter text, inactive platform buttons.
- **Ink Muted** (`#555555`): Unused currently — available for placeholders, disabled states, or the least important metadata.
- **Border Subtle** (`rgba(255,255,255,0.06)`): Tab borders, nav hint borders, progress counter borders. A ghost of a line, visible only against the elevated surfaces.

### The Flat-By-Default Rule

Faptopia has zero `box-shadow` tokens. Zero. Depth is communicated entirely through tonal layering: lighter surfaces sit on top of darker ones. A tab button at `#141414` on the `#0a0a0a` tab bar on `#000000` body — that three-step gray ramp is the entire depth system. No shadow is ever needed.

### The Two-Accent Rule

Reddit orange and 4chan green are the only saturated colors in the system. They signal *platform context*, not interactivity. A red accent means "you are in Reddit content" or "click here for Reddit." A green accent means "you are in 4chan content" or "click here for 4chan." No other color appears — no blue links, no purple visited, no yellow warnings. The room stays dark; color only enters where platform switch is at stake.

## 3. Typography

**UI Font:** Inter (with `-apple-system, BlinkMacSystemFont, system-ui, sans-serif` fallback)

**Character:** Single-family, single-weight hierarchy (400/500/600). No display font, no serif, no personality face — Inter's low-contrast geometric neutrality keeps the interface quiet. The typography serves navigation, not voice. Everything below 1rem because the chrome is subordinate.

### Hierarchy

- **Label / UI** (600, 0.75rem / 12px, 0.08em tracking, uppercase): Gallery toggle buttons (REDDIT / 4CHAN). The only uppercase text in the system.
- **Label / Tab** (500, 0.85rem / 13.6px, normal): Tab-button labels (source names like "r/hotwife"). Body-size text, slightly lighter weight than the button itself.
- **Label / Chip** (500, 0.75rem / 12px, 0.02em tracking): Source-label chip on mobile. Small, semi-transparent, sits over the video.
- **Body / Small** (500, 0.8rem / 12.8px, normal): Nav hint text. The only running-prose-length text in the system.
- **Counter** (600, 0.72rem / 11.5px, 0.04em tracking): Progress counter ("3 / 47"). Smallest UI element — a single number pair, never a sentence.

### No-Display Rule

There is no display typography in this system. No hero heading, no headline, no title text. The content *is* the display element. The gallery opens on a video, not on a nameplate.

## 4. Elevation

Depth is communicated entirely through tonal layering. There are no box-shadows, no drop-shadows, no blur-backdrops used for depth (backdrop-filter appears on the nav hint, source label, and progress counter — these are glass-reveal effects, not depth cues).

The elevation scale has three steps from bottom to top:
1. **Floor** (`#000000`): Body background. The room itself.
2. **Floor +1** (`#0a0a0a`): Tab bar, platform switcher bar, board-tabs bar. Persistent chrome that frames the content area.
3. **Floor +2** (`#141414`): Tab buttons, nav hint background. Interactive elements and overlays that float above the frame.

A fourth step at `#1a1a1a` exists for hover state — it is not a structural elevation, it is a response to cursor proximity.

Backdrop-filter(12px blur) is used on the nav hint, source label, and progress counter to make overlays recede slightly behind their glass surface. This is an ambient effect, not a structural one — the background tint (`rgba(10,10,10,0.85)`) does the work of distinguishing the layer; the blur refines the edge.

### The Flat-By-Default Rule

No `box-shadow` is defined anywhere in the system. A surface's z-position is determined solely by its lightness against the room. A button at `#141414` on the `#0a0a0a` bar is visually lifted without needing a shadow — the three-step ramp is the entire language.

## 5. Components

### Tab Buttons
- **Shape:** Gently rounded pill (8px radius)
- **Resting:** `#141414` background, `#888` text, `rgba(255,255,255,0.06)` border
- **Active:** `#1a1a1a` background, `#f0f0f0` text, `rgba(255,255,255,0.12)` border
- **Hover:** Brightens to `#1a1a1a`, text moves toward primary. Transition: 150ms ease-out
- **Close:** An `×` at 1rem, opacity 0.4 → 1 on hover. Hidden on single-tab (`last-tab`).

### Gallery Toggle
- **Shape:** Gently rounded pill (8px radius), uppercase, 0.08em tracking
- **4chan toggle:** `#2ea44f` background (when viewing Reddit gallery)
- **Reddit toggle:** `#ff4500` background (when viewing 4chan gallery)
- **State:** Text at near-white, `rgba(255,255,255,0.15)` border, 0.85 opacity at rest → 1 on hover
- **Position:** Right side of the tab bar, `margin-left: auto`

### Gallery Navigation
- **Desktop:** Horizontal scroll-snap (x mandatory), full viewport height per item, keyboard-driven (Z/X for prev/next, C for board switch)
- **Mobile:** The board track is a native horizontal scroll-snap container (`scroll-snap-type: x mandatory`, `scroll-snap-stop: always` — a fling advances exactly one board; `touch-action: pan-x pan-y` so vertical drags stay with the feed and horizontal drags reach the track). Each board's content is a Reels-style vertical snap feed — every video gets a full screen slot (`scroll-snap-type: y mandatory`, no gaps between) and is shown whole (`object-fit: contain` — never cropped). The video nearest the viewport's vertical center autoplays with sound; neighbors stay muted, and playback is committed only once the feed settles rather than mid-swipe. Both axes wrap around: a page of items (and each end board) is cloned onto the opposite end and the scroll silently re-anchors onto the real twin, so the feed and the board strip both read as infinite. Keyboard (Z/X/C) scrolls the feed / cycles boards.
- **Scrollbar:** Hidden on gallery containers (CSS `display: none` + `scrollbar-width: none`)

### Platform Switcher (Mobile)
- **Position:** Fixed top bar below safe-area, 44px + safe-top height
- **Style:** Two equally split `<a>` links with subtle gradient backgrounds (red-orange gradient for Reddit, green gradient for 4chan)
- **Active state:** Stronger gradient (35% → 8% alpha), white text
- **Inactive:** Gray text (35% opacity), muted gradient

### Board Tab Dots (Mobile)
- **Shape:** 8px circles, 14px gap, centered in a `linear-gradient(0deg, rgba(0,0,0,0.9), rgba(0,0,0,0.4), transparent)` bar at bottom
- **Active:** 20px-wide rounded pill (4px radius), accent-colored with subtle glow (8px blur radius)
- **Touch target:** Expanded via `::before` pseudo-element to 44px without changing visual size

### Source Label (Mobile)
- **Shape:** Small pill (16px radius), `rgba(0,0,0,0.5)` background, `rgba(255,255,255,0.9)` text, `rgba(255,255,255,0.08)` border
- **Content:** Current board name, prefixed with a 6px accent-dot (platform color, 6px blur-shadow)
- **Position:** Fixed below platform switcher (top: 52px + safe-top), left: 12px
- **Backdrop:** 16px blur — glass effect so it recedes over video content

### Nav Hint (Desktop)
- **Shape:** Rounded bar (12px radius), `rgba(10,10,10,0.85)` background, 12px backdrop-blur
- **Position:** Fixed bottom-center, 20px from bottom, 50% x-transform
- **Content:** "Z / X to navigate · C to switch boards · P to toggle auto-scroll" in 0.8rem secondary text
- **Auto-scroll state:** Border switches to platform accent when active

### Progress Counter
- **Shape:** Small pill (20px radius), `rgba(10,10,10,0.75)` background, 10px backdrop-blur
- **Position:** Fixed below tabs (top: 56px), right: 14px
- **Content:** "N / Total" in 0.72rem, 600 weight, 0.04em tracking
- **Behavior:** Updates on every story/card/video navigation

### Video Player
- **Style:** Full-screen slot per video (`width: 100%; height: 100%`, `object-fit: contain` — whole video always visible, letterboxed on black) on pure black background, one per screen stitched into the mobile snap feed
- **Preload:** `none` by default (first item gets `auto`). JS promotes to `auto` ~400ms before board switch or +-2 items in scroll
- **Audio:** Play muted, unmute on successful play start (iOS/Android autoplay policy compliance)
- **Auto-scroll:** Optional toggle (P key) — on video `ended`, advances to next item

## 6. Do's and Don'ts

### Do:
- **Do** use the three-step gray ramp (black → #0a0a0a → #141414) for all elevation. Never add box-shadow.
- **Do** reserve saturated color exclusively for platform context (Reddit orange / 4chan green). Nothing else gets color.
- **Do** use `text-wrap: balance` on any multi-word UI label longer than 2 words.
- **Do** keep the chrome visually subordinate: tabs are 8px padding pills, nav hint is a single thin bar, progress counter is 11.5px text.
- **Do** use `prefers-reduced-motion: reduce` to strip all transitions and animations. The gallery should work instantly without motion.
- **Do** test at every breakpoint that desktop's scroll-snap tiles fill the viewport and that mobile's feed snaps each full-screen video cleanly onto the viewport without clipping or gaps.
- **Do** use backdrop-filter(blur) as an ambient effect on overlays, not as a primary distinguisher. The tint (rgba layer) does the structural work.
- **Do** safe-area-inset-aware positioning for the platform switcher, source label, and board tabs on notched phones.

### Don't:
- **Don't** use box-shadow anywhere — depth is tonal, not shadowed. No dropdown shadows, no card shadows, no glow effects. The "The Dark Room" north star forbids it.
- **Don't** introduce a third accent color. Reddit orange and 4chan green are the only two saturated colors. No blue links, no purple indicators, no yellow warnings.
- **Don't** use glassmorphism (frosted glass with strong blur + backdrop) as a decorative style. Blur is permitted as a secondary ambient effect on functional overlays only.
- **Don't** over-chrome. Tabs have no icons. No search bar. No filter controls. No sort dropdown. The gallery is a single content stream.
- **Don't** animate with bounce, elastic, or spring easing. The only easing is `cubic-bezier(0.22,1,0.36,1)` for mobile board transitions and `ease-out` for hover states.
- **Don't** use gradient text, gradient borders, or gradient backgrounds on interactive elements (platform switcher gradients are the sole exception — they signal platform subtly).
- **Don't** add a splash screen, onboarding tour, welco