from pathlib import Path

from reportlab.lib import colors
from reportlab.lib.pagesizes import A4
from reportlab.lib.styles import ParagraphStyle, getSampleStyleSheet
from reportlab.lib.units import mm
from reportlab.platypus import ListFlowable, ListItem, Paragraph, SimpleDocTemplate, Spacer


OUTPUT = Path("/Users/Philip/githome/lighting-match-engine-core/output/pdf/lighting-match-engine-core-summary.pdf")


def build_pdf() -> None:
    doc = SimpleDocTemplate(
        str(OUTPUT),
        pagesize=A4,
        leftMargin=14 * mm,
        rightMargin=14 * mm,
        topMargin=12 * mm,
        bottomMargin=12 * mm,
    )

    styles = getSampleStyleSheet()
    title = ParagraphStyle(
        "Title",
        parent=styles["Title"],
        fontName="Helvetica-Bold",
        fontSize=19,
        leading=22,
        textColor=colors.HexColor("#10243E"),
        spaceAfter=4,
    )
    subtitle = ParagraphStyle(
        "Subtitle",
        parent=styles["BodyText"],
        fontName="Helvetica",
        fontSize=8.5,
        leading=10,
        textColor=colors.HexColor("#4A5568"),
        spaceAfter=8,
    )
    h = ParagraphStyle(
        "SectionHeading",
        parent=styles["Heading2"],
        fontName="Helvetica-Bold",
        fontSize=10.5,
        leading=12,
        textColor=colors.HexColor("#0F172A"),
        spaceAfter=3,
        spaceBefore=4,
    )
    body = ParagraphStyle(
        "Body",
        parent=styles["BodyText"],
        fontName="Helvetica",
        fontSize=8.3,
        leading=10.3,
        textColor=colors.black,
        spaceAfter=2,
    )
    bullet = ParagraphStyle(
        "Bullet",
        parent=body,
        leftIndent=0,
        firstLineIndent=0,
        spaceAfter=1.2,
    )
    small = ParagraphStyle(
        "Small",
        parent=body,
        fontSize=7.6,
        leading=9,
        textColor=colors.HexColor("#334155"),
    )

    features = [
        "Two order book implementations: <b>dense</b> and <b>sparse</b>, selected by CLI/config.",
        "Supports <b>limit</b> and <b>market</b> price types in shared order data structures.",
        "Implements <b>continuous matching</b> plus opening/closing <b>call auction</b> flows.",
        "Validates dense-book prices for configured <b>tick</b> and supported range.",
        "Encodes submit/cancel/trade/stats/error messages into fixed <b>64-byte</b> wire packets.",
        "Includes built-in <b>performance timing</b>, result tables, and latency statistics output.",
        "Can pin the matching thread to a CPU core via the cross-platform <b>cpu_affinity</b> module.",
    ]

    architecture = [
        "CLI flags and a few env vars feed <b>config::get_config()</b>, producing <b>AppConfig</b> with instance, product, and order-book settings.",
        "<b>orderbook::factory::build_order_book()</b> creates either a dense or sparse in-memory book.",
        "<b>EngineState</b> is the runtime coordinator: it tracks market phase, routes orders to the active call-auction pool or the continuous order book, and stores match counts/outcomes.",
        "<b>SessionRunner</b> drives a demo session through opening auction, continuous trading, and optional closing auction based on market structure.",
        "<b>protocol::codec</b> serializes orders, cancels, trades, stats, and reject replies; live network I/O/listeners are <b>Not found in repo</b>.",
        "Main data flow in <b>src/main.rs</b>: parse config -> build book -> create engine state -> run demo session -> seed test book -> submit benchmark orders -> print stats / serialize replies on errors.",
    ]

    steps = [
        "Prereq: Rust toolchain version is shown in the repo badge as <b>1.70.0</b>; exact install steps are <b>Not found in repo</b>.",
        "From repo root, run <font name='Courier'>make</font> for the default demo benchmark.",
        "Minimal direct command: <font name='Courier'>cargo run --features match-timing --release -- --prodid 7 --name AAPL --test-order-book-size 50k</font>.",
        "Optional tuning flags in repo code: <font name='Courier'>--order-book dense|sparse --tick N --base-price N --max-levels N --trade-cap N</font>.",
    ]

    story = [
        Paragraph("Lighting Match Engine Core", title),
        Paragraph(
            "One-page repo summary generated from code and docs in this workspace only.",
            subtitle,
        ),
        Paragraph("What It Is", h),
        Paragraph(
            "A Rust matching-engine core for a single product that focuses on low-latency in-memory order matching. "
            "The current repo behaves like a benchmark/demo executable that exercises auctions, continuous trading, and performance reporting.",
            body,
        ),
        Paragraph("Who It's For", h),
        Paragraph(
            "Primary persona: engineers building or evaluating low-latency exchange/trading infrastructure who want a compact matching core they can embed inside a larger stack.",
            body,
        ),
        Paragraph("What It Does", h),
        ListFlowable(
            [ListItem(Paragraph(item, bullet)) for item in features],
            bulletType="bullet",
            start="circle",
            leftIndent=12,
        ),
        Paragraph("How It Works", h),
        ListFlowable(
            [ListItem(Paragraph(item, bullet)) for item in architecture],
            bulletType="bullet",
            start="circle",
            leftIndent=12,
        ),
        Paragraph("How To Run", h),
        ListFlowable(
            [ListItem(Paragraph(item, small)) for item in steps],
            bulletType="bullet",
            start="circle",
            leftIndent=12,
        ),
        Spacer(1, 3),
        Paragraph(
            "Repo note: README mentions UDP multicast and broadcasting, but executable socket-handling code was not located in this repo snapshot.",
            small,
        ),
    ]

    doc.build(story)


if __name__ == "__main__":
    OUTPUT.parent.mkdir(parents=True, exist_ok=True)
    build_pdf()
