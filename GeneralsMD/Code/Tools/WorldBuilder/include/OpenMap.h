#if !defined(AFX_OPENMAP_H__D3FD3B43_B0B7_42F6_BB77_2380A8B9945B__INCLUDED_)
#define AFX_OPENMAP_H__D3FD3B43_B0B7_42F6_BB77_2380A8B9945B__INCLUDED_

#if _MSC_VER > 1000
#pragma once
#endif // _MSC_VER > 1000
// OpenMap.h : header file
//

#define MAP_OPENSAVE_PANEL_SECTION "MapOpenSavePanel"

typedef struct {
	CString filename;
	Bool		browse;
} TOpenMapInfo;

/////////////////////////////////////////////////////////////////////////////
// OpenMap dialog

class OpenMap : public CDialog
{
// Construction
public:
	OpenMap(TOpenMapInfo *pInfo, CWnd* pParent = NULL);   // standard constructor

// Dialog Data
	//{{AFX_DATA(OpenMap)
	enum { IDD = IDD_OPEN_MAP };
		// NOTE: the ClassWizard will add data members here
	//}}AFX_DATA


// Overrides
	// ClassWizard generated virtual function overrides
	//{{AFX_VIRTUAL(OpenMap)
	protected:
	virtual void DoDataExchange(CDataExchange* pDX);    // DDX/DDV support
	//}}AFX_VIRTUAL

// Implementation
protected:
	TOpenMapInfo *m_pInfo;
	void populateMapListbox( Bool systemMaps );
	Bool m_usingSystemDir;
protected:

	// Generated message map functions
	//{{AFX_MSG(OpenMap)
	afx_msg void OnBrowse();
	afx_msg void OnSystemMaps();
	afx_msg void OnUserMaps();
	virtual void OnOK();
	virtual BOOL OnInitDialog();
	afx_msg void OnDblclkOpenList();
	//}}AFX_MSG
	DECLARE_MESSAGE_MAP()
};

//{{AFX_INSERT_LOCATION}}
// Microsoft Visual C++ will insert additional declarations immediately before the previous line.

#endif // !defined(AFX_OPENMAP_H__D3FD3B43_B0B7_42F6_BB77_2380A8B9945B__INCLUDED_)
