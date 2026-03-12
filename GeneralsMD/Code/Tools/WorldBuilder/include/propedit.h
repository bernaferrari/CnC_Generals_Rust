#if !defined(AFX_PROPEDIT_H__93C02F45_592B_4CFD_A092_7445559D26EB__INCLUDED_)
#define AFX_PROPEDIT_H__93C02F45_592B_4CFD_A092_7445559D26EB__INCLUDED_

#if _MSC_VER > 1000
#pragma once
#endif // _MSC_VER > 1000
// propedit.h : header file
//

/////////////////////////////////////////////////////////////////////////////
// PropEdit dialog

class PropEdit : public CDialog
{
// Construction
public:
	PropEdit(AsciiString* key, Dict::DataType* type, AsciiString* value, Bool valueOnly, CWnd *parent = NULL);

// Dialog Data
	//{{AFX_DATA(PropEdit)
	enum { IDD = IDD_PROPEDIT };
		// NOTE: the ClassWizard will add data members here
	//}}AFX_DATA


// Overrides
	// ClassWizard generated virtual function overrides
	//{{AFX_VIRTUAL(PropEdit)
	protected:
	virtual void DoDataExchange(CDataExchange* pDX);    // DDX/DDV support
	//}}AFX_VIRTUAL

// Implementation
protected:
	AsciiString* m_key;
	Dict::DataType* m_type;
	AsciiString* m_value;
	Bool m_valueOnly;
	int m_updating;

	void validate();

	// Generated message map functions
	//{{AFX_MSG(PropEdit)
	afx_msg void OnChangeKeyname();
	afx_msg void OnEditchangeKeytype();
	afx_msg void OnCloseupKeytype();
	afx_msg void OnSelchangeKeytype();
	afx_msg void OnChangeValue();
	virtual BOOL OnInitDialog();
	afx_msg void OnPropbool();
	//}}AFX_MSG
	DECLARE_MESSAGE_MAP()
};

//{{AFX_INSERT_LOCATION}}
// Microsoft Visual C++ will insert additional declarations immediately before the previous line.

#endif // !defined(AFX_PROPEDIT_H__93C02F45_592B_4CFD_A092_7445559D26EB__INCLUDED_)
