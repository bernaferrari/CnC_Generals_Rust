#if !defined(AFX_TERRAINMODAL_H__F013E9EF_2DE1_4084_97A8_5B87466535FC__INCLUDED_)
#define AFX_TERRAINMODAL_H__F013E9EF_2DE1_4084_97A8_5B87466535FC__INCLUDED_

#if _MSC_VER > 1000
#pragma once
#endif // _MSC_VER > 1000
// TerrainModal.h : header file
//

#include "TerrainSwatches.h"
#include "common/AsciiString.h"
class WorldHeightMapEdit;
/////////////////////////////////////////////////////////////////////////////
// TerrainModal dialog

class TerrainModal : public CDialog
{
// Construction
public:
	TerrainModal(AsciiString path, WorldHeightMapEdit *pMap, CWnd* pParent = NULL);   // standard constructor

// Dialog Data
	//{{AFX_DATA(TerrainModal)
	enum { IDD = IDD_TERRAIN_MODAL };
		// NOTE: the ClassWizard will add data members here
	//}}AFX_DATA


// Overrides
	// ClassWizard generated virtual function overrides
	//{{AFX_VIRTUAL(TerrainModal)
	protected:
	virtual void DoDataExchange(CDataExchange* pDX);    // DDX/DDV support
	virtual BOOL OnNotify(WPARAM wParam, LPARAM lParam, LRESULT* pResult);
	//}}AFX_VIRTUAL

// Implementation
protected:

	// Generated message map functions
	//{{AFX_MSG(TerrainModal)
	virtual BOOL OnInitDialog();
	//}}AFX_MSG
	DECLARE_MESSAGE_MAP()

protected:
	Int					m_currentFgTexture;
	AsciiString m_pathToReplace;
	CTreeCtrl		m_terrainTreeView;
	TerrainSwatches		m_terrainSwatches;
	WorldHeightMapEdit *m_map;

protected:
	void addTerrain(char *pPath, Int terrainNdx, HTREEITEM parent);
	HTREEITEM findOrAdd(HTREEITEM parent, char *pLabel);
	void updateLabel(void);
	void updateTextures(void);
	Bool setTerrainTreeViewSelection(HTREEITEM parent, Int selection);

public:
	Int getNewNdx(void) {return m_currentFgTexture;};

};

//{{AFX_INSERT_LOCATION}}
// Microsoft Visual C++ will insert additional declarations immediately before the previous line.

#endif // !defined(AFX_TERRAINMODAL_H__F013E9EF_2DE1_4084_97A8_5B87466535FC__INCLUDED_)
