#if !defined(AFX_MATCHDLG_H__FA868061_4EA7_11D3_B9DA_006097B90D93__INCLUDED_)
#define AFX_MATCHDLG_H__FA868061_4EA7_11D3_B9DA_006097B90D93__INCLUDED_

#if _MSC_VER > 1000
#pragma once
#endif // _MSC_VER > 1000
// MatchDlg.h : header file
//

#include "transDB.h"
#define IDSKIP		100

/////////////////////////////////////////////////////////////////////////////
// CMatchDlg dialog

class CMatchDlg : public CDialog
{
// Construction
public:
	CMatchDlg(CWnd* pParent = NULL);   // standard constructor

// Dialog Data
	//{{AFX_DATA(CMatchDlg)
	enum { IDD = IDD_MATCH };
		// NOTE: the ClassWizard will add data members here
	//}}AFX_DATA


// Overrides
	// ClassWizard generated virtual function overrides
	//{{AFX_VIRTUAL(CMatchDlg)
	protected:
	virtual void DoDataExchange(CDataExchange* pDX);    // DDX/DDV support
	//}}AFX_VIRTUAL

// Implementation
protected:

	// Generated message map functions
	//{{AFX_MSG(CMatchDlg)
	virtual void OnCancel();
	afx_msg void OnNomatch();
	afx_msg void OnMatch();
	virtual BOOL OnInitDialog();
	afx_msg void OnSkip();
	afx_msg void OnSelchangeMatchcombo();
	//}}AFX_MSG
	DECLARE_MESSAGE_MAP()
};

extern BabylonText *MatchingBabylonText;
extern BabylonText *MatchOriginalText;
extern BabylonLabel *MatchLabel;

//{{AFX_INSERT_LOCATION}}
// Microsoft Visual C++ will insert additional declarations immediately before the previous line.

#endif // !defined(AFX_MATCHDLG_H__FA868061_4EA7_11D3_B9DA_006097B90D93__INCLUDED_)
