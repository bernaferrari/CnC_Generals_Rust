#pragma once
#include "Tool.h"

class BorderTool : public Tool
{
	protected:
		enum ModificationType { MOD_TYPE_INVALID, MOD_TYPE_UP, MOD_TYPE_FREE, MOD_TYPE_RIGHT };
		Bool m_mouseDown;
		Bool m_addingNewBorder;
		Int m_modifyBorderNdx;
		ModificationType m_modificationType;
		
	
	public:
		BorderTool();
		~BorderTool();

		Int getToolID(void) {return m_toolID;}
		virtual void setCursor(void);

		virtual void activate(); 
		virtual void deactivate(); 

		virtual Bool followsTerrain(void) { return false;	}

		virtual void mouseMoved(TTrackingMode m, CPoint viewPt, WbView* pView, CWorldBuilderDoc *pDoc);
		virtual void mouseDown(TTrackingMode m, CPoint viewPt, WbView* pView, CWorldBuilderDoc *pDoc);
		virtual void mouseUp(TTrackingMode m, CPoint viewPt, WbView* pView, CWorldBuilderDoc *pDoc);
};