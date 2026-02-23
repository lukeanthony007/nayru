import { cn } from "@/lib/utils";
import { cva } from "class-variance-authority";
import Link from "next/link";

const menuItemBaseStyles = cva(
  "rounded-lg px-3.5 font-medium text-white/40 transition-all duration-200",
  {
    variants: {
      isActive: {
        true: "bg-white/[0.06] text-white/80 hover:bg-white/[0.06]",
        false: "hover:bg-white/[0.04] hover:text-white/60",
      },
    },
    defaultVariants: {
      isActive: false,
    },
  },
);

export function MenuItem(
  props: {
    className?: string;
    children: React.ReactNode;
    isActive: boolean;
    title?: string;
  } & ({ as?: "button"; onClick: () => void } | { as: "link"; href: string }),
) {
  if (props.as === "link") {
    return (
      <Link
        href={props.href}
        title={props.title}
        className={cn(
          menuItemBaseStyles({
            isActive: props.isActive,
            className: "relative block py-2",
          }),
          props.className,
        )}
      >
        {props.children}
      </Link>
    );
  }

  return (
    <button
      onClick={props.onClick}
      title={props.title}
      className={cn(
        menuItemBaseStyles({
          isActive: props.isActive,
          className: "flex w-full items-center gap-3 py-3",
        }),
        props.className,
      )}
    >
      {props.children}
    </button>
  );
}
