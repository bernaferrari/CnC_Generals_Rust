#if !defined(AFX_VIEWDBSII_H__AC697EE4_6D37_4F29_ACC7_8B260A33DA2E__INCLUDED_)
#define AFX_VIEWDBSII_H__AC697EE4_6D37_4F29_ACC7_8B260A33DA2E__INCLUDED_

#if _MSC_VER > 1000
#pragma once
#endif // _MSC_VER > 1000
// VIEWDBSII.h : header file
//

/////////////////////////////////////////////////////////////////////////////
// VIEWDBSII dialog

class VIEWDBSII : public CDialog
{
// Construction
public:
	VIEWDBSII(CWnd* pParent = NULL);   // standard constructor

	void OnClose();
	BOOL OnInitDialog();
	HTREEITEM create_changes_view ( void );
	HTREEITEM VIEWDBSII::create_full_view ( void );

// Dialog Data
	//{{AFX_DATA(VIEWDBSII)
	enum { IDD = IDD_VIEWDBS };
		// NOTE: the ClassWizard will add data members here
	//}}AFX_DATA


// Overrides
	// ClassWizard generated virtual function overrides
	//{{AFX_VIRTUAL(VIEWDBSII)
	protected:
	virtual void DoDataExchange(CDataExchange* pDX);    // DDX/DDV support
	//}}AFX_VIRTUAL

// Implementation
protected:

	// Generated message map functions
	//{{AFX_MSG(VIEWDBSII)
		// NOTE: the ClassWizard will add member functions here
	//}}AFX_MSG
	DECLARE_MESSAGE_MAP()
};

extern int ViewChanges;
//{{AFX_INSERT_LOCATION}}
// Microsoft Visual C++ will insert additional declarations immediately before the previous line.

#endif // !defined(AFX_VIEWDBSII_H__AC697EE4_6D37_4F29_ACC7_8B260A33DA2E__INCLUDED_)
