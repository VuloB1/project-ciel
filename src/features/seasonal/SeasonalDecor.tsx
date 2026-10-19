import type { FestivityArt } from "./festivities";
import { useFestivity } from "./useFestivity";

/** Copies of the 680-wide garland laid side by side: enough to cover very wide windows. */
const GARLAND_COPIES = [0, 1, 2, 3];
const PARTICLE_COUNT = 11;

function Particles({ art }: { art: FestivityArt }) {
  if (art.particles.length === 0) return null;
  return (
    <>
      {Array.from({ length: PARTICLE_COUNT }, (_, i) => {
        const duration = 9 + ((i * 5) % 7);
        const delay = -((i * 3.1) % duration);
        const size = 11 + (i % 3) * 2;
        const crossing = art.motion === "cross";
        return (
          <div
            key={i}
            className="seasonal-particle absolute opacity-60"
            style={{
              left: crossing ? 0 : `${((i * 67 + 23) % 92) + 2}%`,
              top: crossing ? 40 + ((i * 53) % 220) : art.motion === "rise" ? "100%" : 0,
              width: size,
              height: size,
              animation: `seasonal-${art.motion} ${crossing ? 14 + i : duration}s linear ${delay}s infinite`,
            }}
            dangerouslySetInnerHTML={{ __html: `<svg viewBox="0 0 14 14" width="100%" height="100%">${art.particles[i % art.particles.length]}</svg>` }}
          />
        );
      })}
    </>
  );
}

/** Garland along the top of the page, a piece in the bottom-right corner and drifting particles. Never takes clicks. */
export function SeasonalMainDecor() {
  const festivity = useFestivity();
  if (!festivity) return null;
  const { art, motion } = festivity;
  return (
    <div aria-hidden="true" className="pointer-events-none absolute inset-0 z-20 overflow-hidden">
      <div className="absolute inset-x-0 top-0 flex overflow-hidden">
        {GARLAND_COPIES.map((i) => (
          <div key={i} className="w-[680px] shrink-0" dangerouslySetInnerHTML={{ __html: art.top }} />
        ))}
      </div>
      {art.topLeft ? <div className="absolute left-0 top-0 h-[50px] w-[50px]" dangerouslySetInnerHTML={{ __html: art.topLeft }} /> : null}
      <div className="absolute bottom-24 right-4 hidden h-24 w-24 [@media(min-height:640px)]:block" dangerouslySetInnerHTML={{ __html: art.corner }} />
      {motion ? <Particles art={art} /> : null}
    </div>
  );
}

/** Small piece above the sidebar's bottom row; only when the window is tall enough and the sidebar is open. */
export function SeasonalSidebarPiece({ collapsed }: { collapsed: boolean }) {
  const festivity = useFestivity();
  if (!festivity || collapsed) return null;
  return (
    <div aria-hidden="true" className="pointer-events-none hidden shrink-0 justify-center pb-2 [@media(min-height:780px)]:flex">
      <div className="h-14 w-14" dangerouslySetInnerHTML={{ __html: festivity.art.side }} />
    </div>
  );
}

/** Tiny version of the festivity's main piece, next to the app name in the title bar. */
export function SeasonalTitleIcon() {
  const festivity = useFestivity();
  if (!festivity) return null;
  return <span aria-hidden="true" className="h-5 w-5 shrink-0" dangerouslySetInnerHTML={{ __html: festivity.art.icon }} />;
}
