// 歌のクリップの音量の線とフェードの取っ手をドラッグしている間の値（画面だけが先に動く。離したときに setClipField を 1 つ送る）。
// 取っ手と線の部品（HTML）が書き、波形を描く所（WebGL）が読んで、同じコマで波形の大きさとフェードの線を変える。

export type AudioDragValue = {
  /** ClipView.id */
  clip: number;
  gainDb?: number;
  /** ティック */
  fadeIn?: number;
  /** ティック */
  fadeOut?: number;
};

export const audioDrag = $state<{ now: AudioDragValue | null }>({ now: null });
