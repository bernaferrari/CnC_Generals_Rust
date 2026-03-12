
#pragma once

#ifndef __SPLASHSCREEN_H__
#define __SPLASHSCREEN_H__

class SplashScreen : public CDialog
{
	protected:
		CRect m_rect;
		CString m_loadString;
		CFont m_font;

	public:
		SplashScreen();

	public:
		void setTextOutputLocation(const CRect& rect);
		void outputText(UINT nIDString);

	protected:
		afx_msg void OnPaint();

		DECLARE_MESSAGE_MAP()
};

#endif /* __SPLASHSCREEN_H__ */
