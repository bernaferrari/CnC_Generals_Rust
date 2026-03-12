// MyToolBar.h
// Class to do custom toolbar.
// Author: John Ahlquist, April 2001

#pragma once

#ifndef MYTOOLBAR_H
#define MYTOOLBAR_H



/*************************************************************************
**                             CellSizeToolBar
***************************************************************************/
class CellSizeToolBar : public CDialogBar
{
protected:
	static CellSizeToolBar* CellSizeToolBar::m_staticThis;
	CSliderCtrl m_cellSlider;

protected:
	afx_msg void OnVScroll(UINT nSBCode, UINT nPos, CScrollBar* pScrollBar);
	virtual LRESULT WindowProc( UINT message, WPARAM wParam, LPARAM lParam );
	DECLARE_MESSAGE_MAP()

public:
	~CellSizeToolBar(void);
	void SetupSlider(void);
	static void CellSizeChanged(Int cellSize);

};



#endif //MYTOOLBAR_H
