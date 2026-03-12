#if !defined(AFX_OBJECTPREVIEW_H__2EC47AA6_06CA_43D1_9003_15472AE76CE7__INCLUDED_)
#define AFX_OBJECTPREVIEW_H__2EC47AA6_06CA_43D1_9003_15472AE76CE7__INCLUDED_

#if _MSC_VER > 1000
#pragma once
#endif // _MSC_VER > 1000
// ObjectPreview.h : header file
//

#include "lib/BaseType.h"
/////////////////////////////////////////////////////////////////////////////
// ObjectPreview window

class ThingTemplate;

class ObjectPreview : public CWnd
{
// Construction
public:
	ObjectPreview();

// Attributes
public:

// Operations
public:

// Overrides
	// ClassWizard generated virtual function overrides
	//{{AFX_VIRTUAL(ObjectPreview)
	//}}AFX_VIRTUAL

// Implementation
public:
	virtual ~ObjectPreview();

	void SetThingTemplate(const ThingTemplate *tTempl);

	// Generated message map functions
protected:
	//{{AFX_MSG(ObjectPreview)
	afx_msg void OnPaint();
	//}}AFX_MSG
	DECLARE_MESSAGE_MAP()

protected:
	void DrawMyTexture(CDC *pDc, int top, int left, Int width, Int height, UnsignedByte *rgbData);

	const ThingTemplate *m_tTempl;

};

/////////////////////////////////////////////////////////////////////////////

//{{AFX_INSERT_LOCATION}}
// Microsoft Visual C++ will insert additional declarations immediately before the previous line.

#endif // !defined(AFX_OBJECTPREVIEW_H__2EC47AA6_06CA_43D1_9003_15472AE76CE7__INCLUDED_)
