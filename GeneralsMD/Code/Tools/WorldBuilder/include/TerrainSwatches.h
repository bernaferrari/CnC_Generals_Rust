#if !defined(AFX_TERRAINSWATCHES_H__2EC47AA6_06CA_43D1_9003_15472AE76CE7__INCLUDED_)
#define AFX_TERRAINSWATCHES_H__2EC47AA6_06CA_43D1_9003_15472AE76CE7__INCLUDED_

#if _MSC_VER > 1000
#pragma once
#endif // _MSC_VER > 1000
// TerrainSwatches.h : header file
//

#include "lib/BaseType.h"
/////////////////////////////////////////////////////////////////////////////
// TerrainSwatches window

class TerrainSwatches : public CWnd
{
// Construction
public:
	TerrainSwatches();

// Attributes
public:

// Operations
public:

// Overrides
	// ClassWizard generated virtual function overrides
	//{{AFX_VIRTUAL(TerrainSwatches)
	//}}AFX_VIRTUAL

// Implementation
public:
	virtual ~TerrainSwatches();

	// Generated message map functions
protected:
	//{{AFX_MSG(TerrainSwatches)
	afx_msg void OnPaint();
	//}}AFX_MSG
	DECLARE_MESSAGE_MAP()

protected:
	void DrawMyTexture(CDC *pDc, int top, int left, Int width, UnsignedByte *rgbData);

};

/////////////////////////////////////////////////////////////////////////////

//{{AFX_INSERT_LOCATION}}
// Microsoft Visual C++ will insert additional declarations immediately before the previous line.

#endif // !defined(AFX_TERRAINSWATCHES_H__2EC47AA6_06CA_43D1_9003_15472AE76CE7__INCLUDED_)
