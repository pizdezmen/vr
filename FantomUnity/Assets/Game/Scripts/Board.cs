using UnityEngine;
using UnityEngine.UI;

namespace Fantom
{
    /// <summary>Табло над фантомом: очки, подсказка, время забега и вспомогательные цифры.</summary>
    public sealed class Board
    {
        public readonly Transform Root;
        readonly Text title, big, message, line1, line2, line3;

        public Board(Transform parent)
        {
            var go = new GameObject("Board", typeof(Canvas), typeof(CanvasScaler));
            go.transform.SetParent(parent, false);
            Root = go.transform;
            var canvas = go.GetComponent<Canvas>();
            canvas.renderMode = RenderMode.WorldSpace;
            var rect = (RectTransform)Root;
            rect.sizeDelta = new Vector2(1024f, 640f);
            rect.localScale = Vector3.one * (2.4f / 1024f);

            var panel = new GameObject("Panel", typeof(Image));
            panel.transform.SetParent(Root, false);
            var panelRect = (RectTransform)panel.transform;
            panelRect.anchorMin = Vector2.zero;
            panelRect.anchorMax = Vector2.one;
            panelRect.offsetMin = Vector2.zero;
            panelRect.offsetMax = Vector2.zero;
            var image = panel.GetComponent<Image>();
            image.color = new Color(0.01f, 0.04f, 0.13f, 0.75f);
            image.raycastTarget = false;

            var font = Resources.GetBuiltinResource<Font>("LegacyRuntime.ttf");
            title = MakeText(font, 258f, 40, new Color(0.5f, 0.72f, 1f), FontStyle.Bold);
            big = MakeText(font, 150f, 150, Color.white, FontStyle.Bold);
            message = MakeText(font, 22f, 44, new Color(1f, 0.82f, 0.48f), FontStyle.Bold);
            line1 = MakeText(font, -70f, 42, new Color(0.81f, 0.89f, 1f), FontStyle.Normal);
            line2 = MakeText(font, -150f, 38, new Color(0.56f, 0.7f, 0.91f), FontStyle.Normal);
            line3 = MakeText(font, -235f, 32, new Color(0.45f, 0.58f, 0.8f), FontStyle.Normal);
            SetVisible(false);
        }

        Text MakeText(Font font, float y, int size, Color color, FontStyle style)
        {
            var go = new GameObject("Text", typeof(Text));
            go.transform.SetParent(Root, false);
            var rect = (RectTransform)go.transform;
            rect.sizeDelta = new Vector2(1000f, size * 1.4f);
            rect.anchoredPosition = new Vector2(0f, y);
            var text = go.GetComponent<Text>();
            text.font = font;
            text.fontSize = size;
            text.fontStyle = style;
            text.color = color;
            text.alignment = TextAnchor.MiddleCenter;
            text.horizontalOverflow = HorizontalWrapMode.Overflow;
            text.verticalOverflow = VerticalWrapMode.Overflow;
            text.raycastTarget = false;
            return text;
        }

        public void SetVisible(bool visible) => Root.gameObject.SetActive(visible);

        public void Set(string titleText, string bigText, string messageText, string l1, string l2, string l3)
        {
            title.text = titleText;
            big.text = bigText;
            message.text = messageText;
            line1.text = l1;
            line2.text = l2;
            line3.text = l3;
        }
    }
}
