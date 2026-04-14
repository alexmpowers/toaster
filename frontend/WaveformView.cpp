#include "WaveformView.h"

#include <QMouseEvent>
#include <QPainter>

#include <algorithm>

namespace {

void drawRangeOverlay(QPainter *painter, const QRect &drawRect, qint64 durationUs,
                      const toaster_time_range_t &range, const QColor &color)
{
  if (!painter || durationUs <= 0 || range.end_us <= range.start_us)
    return;

  auto positionFor = [durationUs, &drawRect](qint64 value) {
    double ratio = std::clamp(static_cast<double>(value) / static_cast<double>(durationUs), 0.0, 1.0);
    return drawRect.left() + static_cast<int>(ratio * drawRect.width());
  };

  int left = positionFor(range.start_us);
  int right = positionFor(range.end_us);

  if (right <= left)
    right = left + 1;

  painter->fillRect(QRect(left, drawRect.top(), right - left, drawRect.height()), color);
}

}  // namespace

WaveformView::WaveformView(QWidget *parent) : QWidget(parent)
{
  setMinimumHeight(180);
  setMouseTracking(true);
}

void WaveformView::clear()
{
  m_waveformImage = QImage();
  m_durationUs = 0;
  m_playheadUs = 0;
  m_hasSelectedRange = false;
  m_deletedRanges.clear();
  m_cutRanges.clear();
  m_silencedRanges.clear();
  update();
}

void WaveformView::setWaveformImage(const QImage &image)
{
  m_waveformImage = image;
  update();
}

void WaveformView::setDurationUs(qint64 durationUs)
{
  m_durationUs = std::max<qint64>(0, durationUs);
  update();
}

void WaveformView::setPlayheadUs(qint64 playheadUs)
{
  m_playheadUs = std::max<qint64>(0, playheadUs);
  update();
}

void WaveformView::setDeletedRanges(const QVector<toaster_time_range_t> &ranges)
{
  m_deletedRanges = ranges;
  update();
}

void WaveformView::setCutRanges(const QVector<toaster_time_range_t> &ranges)
{
  m_cutRanges = ranges;
  update();
}

void WaveformView::setSilencedRanges(const QVector<toaster_time_range_t> &ranges)
{
  m_silencedRanges = ranges;
  update();
}

void WaveformView::setSelectedRange(const toaster_time_range_t &range)
{
  m_selectedRange = range;
  m_hasSelectedRange = true;
  update();
}

void WaveformView::clearSelectedRange()
{
  m_hasSelectedRange = false;
  update();
}

QRect WaveformView::contentRect() const
{
  return rect().adjusted(10, 10, -10, -10);
}

int WaveformView::xForTime(qint64 positionUs, const QRect &drawRect) const
{
  if (m_durationUs <= 0)
    return drawRect.left();

  double ratio =
    std::clamp(static_cast<double>(positionUs) / static_cast<double>(m_durationUs), 0.0, 1.0);
  return drawRect.left() + static_cast<int>(ratio * drawRect.width());
}

qint64 WaveformView::timeForX(int x, const QRect &drawRect) const
{
  if (m_durationUs <= 0 || drawRect.width() <= 0)
    return 0;

  int clampedX = std::clamp(x, drawRect.left(), drawRect.right());
  double ratio = static_cast<double>(clampedX - drawRect.left()) / static_cast<double>(drawRect.width());
  return static_cast<qint64>(ratio * static_cast<double>(m_durationUs));
}

void WaveformView::paintEvent(QPaintEvent *event)
{
  Q_UNUSED(event);

  QPainter painter(this);
  QRect drawRect = contentRect();

  painter.fillRect(rect(), QColor(24, 26, 32));
  painter.fillRect(drawRect, QColor(16, 18, 24));

  if (!m_waveformImage.isNull()) {
    painter.drawImage(drawRect,
                      m_waveformImage.scaled(drawRect.size(), Qt::IgnoreAspectRatio,
                                             Qt::SmoothTransformation));
  } else {
    painter.setPen(QColor(190, 194, 201));
    painter.drawText(drawRect, Qt::AlignCenter, "Waveform unavailable for current media");
  }

  for (const toaster_time_range_t &range : m_deletedRanges)
    drawRangeOverlay(&painter, drawRect, m_durationUs, range, QColor(255, 157, 66, 70));

  for (const toaster_time_range_t &range : m_cutRanges)
    drawRangeOverlay(&painter, drawRect, m_durationUs, range, QColor(225, 64, 64, 95));

  for (const toaster_time_range_t &range : m_silencedRanges)
    drawRangeOverlay(&painter, drawRect, m_durationUs, range, QColor(64, 128, 255, 80));

  if (m_hasSelectedRange)
    drawRangeOverlay(&painter, drawRect, m_durationUs, m_selectedRange, QColor(255, 224, 110, 70));

  if (m_durationUs > 0) {
    int playheadX = xForTime(m_playheadUs, drawRect);
    painter.setPen(QPen(QColor(255, 96, 96), 2));
    painter.drawLine(playheadX, drawRect.top(), playheadX, drawRect.bottom());
  }

  painter.setPen(QColor(72, 77, 88));
  painter.drawRect(drawRect.adjusted(0, 0, -1, -1));
}

void WaveformView::mousePressEvent(QMouseEvent *event)
{
  if (event->button() == Qt::LeftButton && m_durationUs > 0)
    emit seekRequested(timeForX(event->position().x(), contentRect()));

  QWidget::mousePressEvent(event);
}
