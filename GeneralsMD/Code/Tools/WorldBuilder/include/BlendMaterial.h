#if !defined(AFX_BlendMaterial_H__D3FF66C5_711D_4DAC_8A29_5EAAB5C3A23E__INCLUDED_)
#define AFX_BlendMaterial_H__D3FF66C5_711D_4DAC_8A29_5EAAB5C3A23E__INCLUDED_

#if _MSC_VER > 1000
#pragma once
#endif // _MSC_VER > 1000
// BlendMaterial.h : header file
//

#include "WBPopupSlider.h"
#include "TerrainSwatches.h"
#include "OptionsPanel.h"
class WorldHeightMapEdit;
/////////////////////////////////////////////////////////////////////////////
// BlendMaterial dialog

class BlendMaterial : public COptionsPanel
{
// Construction
public:
	BlendMaterial(CWnd* pParent = NULL);   // standard constructor

// Dialog Data
	//{{AFX_DATA(BlendMaterial)
	enum { IDD = IDD_BLEND_MATERIAL };
		// NOTE: the ClassWizard will add data members here
	//}}AFX_DATA


// Overrides
	// ClassWizard generated virtual function overrides
	//{{AFX_VIRTUAL(BlendMaterial)
	protected:
	virtual void DoDataExchange(CDataExchange* pDX);    // DDX/DDV support
	virtual void OnOK(){return;};  ///< Modeless dialogs don't OK, so eat this for modeless.
	virtual void OnCancel(){return;}; ///< Modeless dialogs don't close on ESC, so eat this for modeless.
	virtual BOOL OnNotify(WPARAM wParam, LPARAM lParam, LRESULT* pResult);
	//}}AFX_VIRTUAL

// Implementation
protected:
	enum {MIN_TILE_SIZE=2, MAX_TILE_SIZE = 100};
	// Generated message map functions
	//{{AFX_MSG(BlendMaterial)
	virtual BOOL OnInitDialog();
	//}}AFX_MSG
	DECLARE_MESSAGE_MAP()


protected:
	static BlendMaterial	*m_staticThis;
	Bool										m_updating;
	static Int							m_currentBlendTexture;
	CTreeCtrl								m_terrainTreeView;

protected:
	void updateTextures(void);
	void addTerrain(const char *pPath, Int terrainNdx, HTREEITEM parent);
	HTREEITEM findOrAdd(HTREEITEM parent, char *pLabel);

public:
	static Int getBlendTexClass(void) {return m_currentBlendTexture;}

	static void setBlendTexClass(Int texClass);

public:
	Bool setTerrainTreeViewSelection(HTREEITEM parent, Int selection);

}; 

//{{AFX_INSERT_LOCATION}}
// Microsoft Visual C++ will insert additional declarations immediately before the previous line.

#endif // !defined(AFX_BlendMaterial_H__D3FF66C5_711D_4DAC_8A29_5EAAB5C3A23E__INCLUDED_)
