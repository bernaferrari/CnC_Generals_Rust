
#include "StdAfx.h"
#include "SplashScreen.h"

//-------------------------------------------------------------------------------------------------
SplashScreen::SplashScreen()
{
	m_rect.left = 0;
	m_rect.right = 0;
	m_rect.top = 0;
	m_rect.bottom = 0;

	m_loadString = "Cock & Beer";


	LOGFONT lf;
	lf.lfHeight = 12;
	lf.lfWidth = 0;
	lf.lfEscapement = 0;
	lf.lfOrientation = 0;
	lf.lfWeight = FW_NORMAL;
	lf.lfItalic = FALSE;
	lf.lfUnderline = FALSE;
	lf.lfStrikeOut = FALSE;
	lf.lfCharSet = ANSI_CHARSET;
	lf.lfOutPrecision = OUT_DEFAULT_PRECIS;
	lf.lfClipPrecision = CLIP_DEFAULT_PRECIS;
	lf.lfQuality = DEFAULT_QUALITY;
	lf.lfPitchAndFamily = DEFAULT_PITCH | FF_DONTCARE;
	strcpy(lf.lfFaceName, "Arial");
	
	m_font.CreateFontIndirect(&lf);
}

//-------------------------------------------------------------------------------------------------
void SplashScreen::setTextOutputLocation(const CRect& rect)
{
	m_rect = rect;
}

//-------------------------------------------------------------------------------------------------
void SplashScreen::outputText(UINT nIDString)
{
	CString str;
	if (!str.LoadString(nIDString)) {
		return;
	}

	m_loadString = str;
	
	RedrawWindow(&m_rect, NULL);
}

//-------------------------------------------------------------------------------------------------
void SplashScreen::OnPaint()
{
	// we're extending the default behavior
	CDialog::OnPaint();

	
	CDC *dc = GetDC();
	
	// Save off the old font
	CFont *oldFont = dc->SelectObject(&m_font);
	COLORREF oldRef = dc->SetTextColor(0x00000000);
	
//	dc->DrawText(m_loadString, m_rect, DT_VCENTER | DT_LEFT);
	
	// restore the old font
	dc->SelectObject(oldFont);
	dc->SetTextColor(oldRef);
}

BEGIN_MESSAGE_MAP(SplashScreen, CDialog)
	ON_WM_PAINT()
END_MESSAGE_MAP()
