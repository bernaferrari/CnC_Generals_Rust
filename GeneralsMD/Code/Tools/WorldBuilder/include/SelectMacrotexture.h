#if !defined(AFX_SELECTMACROTEXTURE_H__0AB61BFA_3A67_40CB_A38F_7067F6BA352B__INCLUDED_)
#define AFX_SELECTMACROTEXTURE_H__0AB61BFA_3A67_40CB_A38F_7067F6BA352B__INCLUDED_

#if _MSC_VER > 1000
#pragma once
#endif // _MSC_VER > 1000
// SelectMacrotexture.h : header file
//

/////////////////////////////////////////////////////////////////////////////
// SelectMacrotexture dialog

class SelectMacrotexture : public CDialog
{
// Construction
public:
	SelectMacrotexture(CWnd* pParent = NULL);   // standard constructor

// Dialog Data
	//{{AFX_DATA(SelectMacrotexture)
	enum { IDD = IDD_MACRO_TEXTURE };
		// NOTE: the ClassWizard will add data members here
	//}}AFX_DATA


// Overrides
	// ClassWizard generated virtual function overrides
	//{{AFX_VIRTUAL(SelectMacrotexture)
	protected:
	virtual void DoDataExchange(CDataExchange* pDX);    // DDX/DDV support
	virtual BOOL OnNotify(WPARAM wParam, LPARAM lParam, LRESULT* pResult);
	//}}AFX_VIRTUAL
	
protected:
	CTreeCtrl		m_textureTreeView;


// Implementation
protected:

	// Generated message map functions
	//{{AFX_MSG(SelectMacrotexture)
	virtual BOOL OnInitDialog();
	//}}AFX_MSG
	DECLARE_MESSAGE_MAP()
};

//{{AFX_INSERT_LOCATION}}
// Microsoft Visual C++ will insert additional declarations immediately before the previous line.

#endif // !defined(AFX_SELECTMACROTEXTURE_H__0AB61BFA_3A67_40CB_A38F_7067F6BA352B__INCLUDED_)
