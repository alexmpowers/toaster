#pragma once

#include <QImage>
#include <QWidget>

#include <QVector>

extern "C" {
#include "toaster.h"
}

class WaveformView : public QWidget {
  Q_OBJECT

public:
  explicit WaveformView(QWidget *parent = nullptr);

  void clear();
  void setWaveformImage(const QImage &image);
  void setDurationUs(qint64 durationUs);
  void setPlayheadUs(qint64 playheadUs);
  void setDeletedRanges(const QVector<toaster_time_range_t> &ranges);
  void setCutRanges(const QVector<toaster_time_range_t> &ranges);
  void setSilencedRanges(const QVector<toaster_time_range_t> &ranges);
  void setSelectedRange(const toaster_time_range_t &range);
  void clearSelectedRange();

signals:
  void seekRequested(qint64 positionUs);

protected:
  void paintEvent(QPaintEvent *event) override;
  void mousePressEvent(QMouseEvent *event) override;

private:
  QRect contentRect() const;
  int xForTime(qint64 positionUs, const QRect &drawRect) const;
  qint64 timeForX(int x, const QRect &drawRect) const;

  QImage m_waveformImage;
  qint64 m_durationUs = 0;
  qint64 m_playheadUs = 0;
  bool m_hasSelectedRange = false;
  toaster_time_range_t m_selectedRange{};
  QVector<toaster_time_range_t> m_deletedRanges;
  QVector<toaster_time_range_t> m_cutRanges;
  QVector<toaster_time_range_t> m_silencedRanges;
};
