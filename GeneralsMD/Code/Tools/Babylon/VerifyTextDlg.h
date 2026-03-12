#if !defined(AFX_VERIFYTEXTDLG_H__39DB1C81_6B90_11D3_B9DA_006097B90D93__INCLUDED_)
#define AFX_VERIFYTEXTDLG_H__39DB1C81_6B90_11D3_B9DA_006097B90D93__INCLUDED_

#if _MSC_VER > 1000
#pragma once
#endif // _MSC_VER > 1000
// VerifyTextDlg.h : header file
//

/////////////////////////////////////////////////////////////////////////////
// CVerifyTextDlg dialog

class CVerifyTextDlg : public CDialog
{
// Construction
	char *m_trans;
	char *m_orig;
public:
	CVerifyTextDlg(char *trans, char *orig, CWnd* pParent = NULL);   // standard constructor

// Dialog Data
	//{{AFX_DATA(CVerifyTextDlg)
	enum { IDD = IDD_VERIFYTEXT };
		// NOTE: the ClassWizard will add data members here
	//}}AFX_DATA


// Overrides
	// ClassWizard generated virtual function overrides
	//{{AFX_VIRTUAL(CVerifyTextDlg)
	protected:
	virtual void DoDataExchange(CDataExchange* pDX);    // DDX/DDV support
	//}}AFX_VIRTUAL

// Implementation
protected:

	// Generated message map functions
	//{{AFX_MSG(CVerifyTextDlg)
	afx_msg void OnNomatch();
	afx_msg void OnMatch();
	virtual BOOL OnInitDialog();
	//}}AFX_MSG
	DECLARE_MESSAGE_MAP()
};

//{{AFX_INSERT_LOCATION}}
// Microsoft Visual C++ will insert additional declarations immediately before the previous line.

#endif // !defined(AFX_VERIFYTEXTDLG_H__39DB1C81_6B90_11D3_B9DA_006097B90D93__INCLUDED_)
