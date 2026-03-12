#if !defined(AFX_RETRANSLATEDLG_H__19E25401_0ECB_11D4_B9DB_006097B90D93__INCLUDED_)
#define AFX_RETRANSLATEDLG_H__19E25401_0ECB_11D4_B9DB_006097B90D93__INCLUDED_

#if _MSC_VER > 1000
#pragma once
#endif // _MSC_VER > 1000
// RetranslateDlg.h : header file
//

#include "transdb.h"
#define IDSKIP		100
/////////////////////////////////////////////////////////////////////////////
// RetranslateDlg dialog

class RetranslateDlg : public CDialog
{
// Construction
public:

	BabylonText *newtext;
	BabylonText *oldtext;

	RetranslateDlg(CWnd* pParent = NULL);   // standard constructor

// Dialog Data
	//{{AFX_DATA(RetranslateDlg)
	enum { IDD = IDD_RETRANSLATE };
		// NOTE: the ClassWizard will add data members here
	//}}AFX_DATA


// Overrides
	// ClassWizard generated virtual function overrides
	//{{AFX_VIRTUAL(RetranslateDlg)
	protected:
	virtual void DoDataExchange(CDataExchange* pDX);    // DDX/DDV support
	//}}AFX_VIRTUAL

// Implementation
protected:

	// Generated message map functions
	//{{AFX_MSG(RetranslateDlg)
	virtual BOOL OnInitDialog();
	afx_msg void OnCancelMode();
	afx_msg void OnRetranslate();
	afx_msg void OnSkip();
	afx_msg void OnNoRetranslate();
	afx_msg void OnSkipAll();
	//}}AFX_MSG
	DECLARE_MESSAGE_MAP()
};

//{{AFX_INSERT_LOCATION}}
// Microsoft Visual C++ will insert additional declarations immediately before the previous line.

#endif // !defined(AFX_RETRANSLATEDLG_H__19E25401_0ECB_11D4_B9DB_006097B90D93__INCLUDED_)
