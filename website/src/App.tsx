import { Footer } from './components/Footer.tsx';
import { Nav } from './components/Nav.tsx';
import { Ci } from './components/sections/Ci.tsx';
import { Defect } from './components/sections/Defect.tsx';
import { Files } from './components/sections/Files.tsx';
import { Hero } from './components/sections/Hero.tsx';
import { Install } from './components/sections/Install.tsx';
import { Projects } from './components/sections/Projects.tsx';
import { Rule } from './components/sections/Rule.tsx';
import { Safety } from './components/sections/Safety.tsx';
import { Versions } from './components/sections/Versions.tsx';

export function App() {
  return (
    <>
      <a
        href="#top"
        className="focus:bg-signal focus:text-signal-ink sr-only focus:not-sr-only focus:fixed focus:top-3 focus:left-3 focus:z-[60] focus:rounded-full focus:px-4 focus:py-2"
      >
        Skip to content
      </a>
      <Nav />
      <main>
        <Hero />
        <Defect />
        <Rule />
        <Files />
        <Versions />
        <Ci />
        <Projects />
        <Safety />
        <Install />
      </main>
      <Footer />
    </>
  );
}
