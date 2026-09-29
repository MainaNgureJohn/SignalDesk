const MARKS = [
  { name: "Pollen", symbol: "✦", className: "sd-landing-pollen" },
  { name: "Fizz", symbol: "↗", className: "sd-landing-fizz" },
  { name: "Honey", symbol: "▤", className: "sd-landing-honey" },
] as const;

export function LandingSignal() {
  return (
    <div aria-hidden className="sd-landing-field">
      <div className="sd-landing-orbit" />
      {MARKS.map(({ name, symbol, className }) => (
        <div className={`sd-landing-agent ${className}`} key={name}>
          <span>{symbol}</span>
          <small>{name}</small>
        </div>
      ))}
    </div>
  );
}
