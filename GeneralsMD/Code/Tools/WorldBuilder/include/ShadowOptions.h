#if !defined(AFX_SHADOWOPTIONS_H__25CB23E2_9E42_44D4_BE83_4B89F87746DE__INCLUDED_)
#define AFX_SHADOWOPTIONS_H__25CB23E2_9E42_44D4_BE83_4B89F87746DE__INCLUDED_

#if _MSC_VER > 1000
#pragma once
#endif // _MSC_VER > 1000
// ShadowOptions.h : header file
//

/////////////////////////////////////////////////////////////////////////////
// ShadowOptions dialog

class ShadowOptions : public CDialog
{
// Construction
public:
	ShadowOptions(CWnd* pParent = NULL);   // standard constructor

// Dialog Data
	//{{AFX_DATA(ShadowOptions)
	enum { IDD = IDD_SHADOW_OPTIONS };
		// NOTE: the ClassWizard will add data members here
	//}}AFX_DATA


// Overrides
	// ClassWizard generated virtual function overrides
	//{{AFX_VIRTUAL(ShadowOptions)
	protected:
	virtual void DoDataExchange(CDataExchange* pDX);    // DDX/DDV support
	//}}AFX_VIRTUAL

// Implementation
protected:
	Real m_red;
	Real m_green;
	Real m_blue;
	Real m_intensity;

protected:
	void setShadowColor(void);

	// Generated message map functions
	//{{AFX_MSG(ShadowOptions)
	afx_msg void OnChangeAlphaEdit();
	afx_msg void OnChangeBaEdit();
	afx_msg void OnChangeGaEdit();
	afx_msg void OnChangeRaEdit();
	virtual BOOL OnInitDialog();
	//}}AFX_MSG
	DECLARE_MESSAGE_MAP()
};

//{{AFX_INSERT_LOCATION}}
// Microsoft Visual C++ will insert additional declarations immediately before the previous line.

#endif // !defined(AFX_SHADOWOPTIONS_H__25CB23E2_9E42_44D4_BE83_4B89F87746DE__INCLUDED_)
