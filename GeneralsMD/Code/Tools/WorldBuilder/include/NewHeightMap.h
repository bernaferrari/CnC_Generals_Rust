#if !defined(AFX_NEWHEIGHTMAP_H__239CD6A3_2919_11D5_8CE0_00010297BBAC__INCLUDED_)
#define AFX_NEWHEIGHTMAP_H__239CD6A3_2919_11D5_8CE0_00010297BBAC__INCLUDED_

#if _MSC_VER > 1000
#pragma once
#endif // _MSC_VER > 1000
// NewHeightMap.h : header file
//

#include "Lib/BaseType.h"

typedef struct {
	Int xExtent;
	Int yExtent;
	Int initialHeight;
	Int borderWidth;
	Bool forResize;			///< If for resize, then the anchor bools are set.
	Bool anchorTop;			///< It can be anchor top, or anchor bottom, or neither, but not both.
	Bool anchorBottom;
	Bool anchorLeft;			///< It can be anchor left, or anchor right, or neither but not both.
	Bool anchorRight;
} TNewHeightInfo;


/////////////////////////////////////////////////////////////////////////////
// CNewHeightMap dialog

class CNewHeightMap : public CDialog
{
// Construction
public:
	CNewHeightMap(TNewHeightInfo *hiP, const char *label, CWnd* pParent = NULL);   // standard constructor

	void GetHeightInfo(TNewHeightInfo *hiP) {*hiP = mHeightInfo; };
// Dialog Data
	//{{AFX_DATA(CNewHeightMap)
	enum { IDD = IDD_NewHeightMap };
		// NOTE: the ClassWizard will add data members here
	//}}AFX_DATA


// Overrides
	// ClassWizard generated virtual function overrides
	//{{AFX_VIRTUAL(CNewHeightMap)
	protected:
	virtual void DoDataExchange(CDataExchange* pDX);    // DDX/DDV support
	virtual void OnOK();
	virtual BOOL OnCommand(WPARAM wParam, LPARAM lParam);
	//}}AFX_VIRTUAL

// Implementation
protected:
	TNewHeightInfo mHeightInfo;
	const char *m_label;

protected:
	Bool doAnchorButton(Int buttonID);

protected:

	// Generated message map functions
	//{{AFX_MSG(CNewHeightMap)
	virtual BOOL OnInitDialog();
	//}}AFX_MSG
	DECLARE_MESSAGE_MAP()
};

//{{AFX_INSERT_LOCATION}}
// Microsoft Visual C++ will insert additional declarations immediately before the previous line.

#endif // !defined(AFX_NEWHEIGHTMAP_H__239CD6A3_2919_11D5_8CE0_00010297BBAC__INCLUDED_)
