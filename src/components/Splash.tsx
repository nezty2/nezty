import { AnimatePresence, motion } from "framer-motion";
import { Zap } from "lucide-react";

/** Openingsanimatie: het logo springt in beeld met een gloed, daarna vervaagt het scherm naar de app. */
export function Splash({ show }: { show: boolean }) {
  return (
    <AnimatePresence>
      {show && (
        <motion.div
          key="splash"
          className="fixed inset-0 z-[100] flex items-center justify-center bg-bg"
          initial={{ opacity: 1 }}
          exit={{ opacity: 0, transition: { duration: 0.45, ease: "easeOut" } }}
        >
          <div className="relative flex flex-col items-center">
            <motion.div
              className="absolute top-[-40px] h-44 w-44 rounded-full"
              style={{ background: "radial-gradient(circle, var(--accent), transparent 68%)", filter: "blur(30px)" }}
              initial={{ scale: 0.6, opacity: 0 }}
              animate={{ scale: [0.6, 1.15, 1], opacity: [0, 0.55, 0.35] }}
              transition={{ duration: 1.1, ease: "easeOut" }}
            />
            <motion.div
              className="relative flex h-16 w-16 items-center justify-center rounded-2xl btn-primary"
              initial={{ scale: 0.5, opacity: 0, rotate: -14 }}
              animate={{ scale: 1, opacity: 1, rotate: 0 }}
              transition={{ type: "spring", stiffness: 260, damping: 17 }}
            >
              <Zap className="h-8 w-8 text-white" fill="currentColor" />
            </motion.div>
            <motion.div
              className="relative mt-4 text-xl font-semibold tracking-[0.2em] text-ink"
              initial={{ opacity: 0, y: 8, letterSpacing: "0.5em" }}
              animate={{ opacity: 1, y: 0, letterSpacing: "0.2em" }}
              transition={{ delay: 0.2, duration: 0.5, ease: [0.22, 1, 0.36, 1] }}
            >
              nezty
            </motion.div>
            <div className="relative mt-4 h-0.5 w-28 overflow-hidden rounded-full bg-line">
              <motion.div
                className="h-full w-1/2 rounded-full bg-accent"
                initial={{ x: "-100%" }}
                animate={{ x: "200%" }}
                transition={{ repeat: Infinity, duration: 1.1, ease: "easeInOut" }}
              />
            </div>
          </div>
        </motion.div>
      )}
    </AnimatePresence>
  );
}
