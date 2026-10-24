<script lang="ts">
  export let name = 'pad';
  export let size = 20;
  const paths: Record<string, string> = {
    pad: 'M7 6h10a4 4 0 0 1 4 3l2 9a2 2 0 0 1-3.4 1.8L16 16H8l-3.6 3.8A2 2 0 0 1 1 18l2-9a4 4 0 0 1 4-3ZM7 9v5M4.5 11.5h5M16 10h.01M19 13h.01',
    profiles: 'M4 4h6l2 3h8v13H4V4ZM4 10h16',
    remap: 'M4 7h15m-4-4 4 4-4 4M20 17H5m4-4-4 4 4 4',
    macros: 'm9 5 10 7L9 19V5ZM4 5v14',
    axes: 'M12 3v18M3 12h18m-7-7-2-2-2 2m-5 5-2 2 2 2m9 5-2 2-2-2m9-9 2 2-2 2',
    diagnostics: 'M3 12h4l3-8 4 16 3-8h4',
    settings:
      'M12 8a4 4 0 1 0 0 8 4 4 0 0 0 0-8ZM9 3h6l1 3 3 1 2 5-2 5-3 1-1 3H9l-1-3-3-1-2-5 2-5 3-1 1-3Z',
    chevron: 'm9 5 7 7-7 7',
    down: 'm6 9 6 6 6-6',
    check: 'm5 12 4 4L19 6',
    plus: 'M12 5v14M5 12h14',
    undo: 'M8 5 3 10l5 5M3 10h11a6 6 0 0 1 0 12',
    download: 'M12 3v12m-5-5 5 5 5-5M4 16v5h16v-5',
    upload: 'M12 16V4m-5 5 5-5 5 5M4 16v5h16v-5',
    pause: 'M8 5v14M16 5v14',
    play: 'm8 4 12 8-12 8V4Z',
    battery: 'M3 7h16v10H3V7Zm19 3v4M6 10v4m3-4v4m3-4v4',
    bolt: 'm13 2-9 12h7l-1 8 10-13h-8l1-7Z',
    link: 'm9 15 6-6M8 16l-2 2a4 4 0 0 1-6-6l4-4a4 4 0 0 1 6 0m4 0 2-2a4 4 0 0 1 6 6l-4 4a4 4 0 0 1-6 0',
    info: 'M12 11v6m0-10h.01M22 12a10 10 0 1 1-20 0 10 10 0 0 1 20 0Z',
    close: 'm6 6 12 12M6 18 18 6',
    record: 'M20 12a8 8 0 1 1-16 0 8 8 0 0 1 16 0Z',
    trash: 'M3 6h18M9 6V3h6v3M6 6l1 15h10l1-15M10 10v7m4-7v7',
    grip: 'M8 5h.01M16 5h.01M8 12h.01M16 12h.01M8 19h.01M16 19h.01',
    sun: 'M12 8a4 4 0 1 0 0 8 4 4 0 0 0 0-8ZM12 1v3m0 16v3M1 12h3m16 0h3M4 4l2 2m12 12 2 2M20 4l-2 2M6 18l-2 2',
  };
</script>

<svg
  width={size}
  height={size}
  viewBox="0 0 24 24"
  fill="none"
  stroke="currentColor"
  stroke-width="1.6"
  stroke-linecap="round"
  stroke-linejoin="round"
  aria-hidden="true"><path d={paths[name] ?? paths.pad} /></svg
>
