#ifndef GROVEOPTIONS_H
#define GROVEOPTIONS_H

#pragma once

#include <map>
#include <vector>
#include "worldbuilder.h"
#include "OptionsPanel.h"
#include "Common/STLTypedefs.h"

// Used to store Display names in conjunction with internal names.
typedef std::pair<AsciiString, UnicodeString>							PairNameDisplayName;
typedef std::vector<PairNameDisplayName>									VecPairNameDisplayName;
typedef std::vector<PairNameDisplayName>::iterator				VecPairNameDisplayNameIt;


// This is a utility function useful to get a display string from a pair of AsciiString
// UnicodeStrings. It attempts to use the UnicodeString, and if that fails then turns
// to the AsciiString
// As a last resort, it returns the EmptyString.
UnicodeString GetDisplayNameFromPair(const PairNameDisplayName *pNamePair);

class GroveOptions : public COptionsPanel
{
	protected:
		std::vector<std::pair<Int, Int> >	mVecGroup;
		VecPairNameDisplayName mVecDisplayNames;

		Int	mNumTrees;
	
	public:
		GroveOptions(CWnd* pParent = NULL);
		~GroveOptions();
		void makeMain(void);

		virtual BOOL OnInitDialog();
		int getNumTrees(void);
		int getNumType(int type);
		AsciiString getTypeName(int type);
		int getTotalTreePerc(void);
		Bool getCanPlaceInWater(void);
		Bool getCanPlaceOnCliffs(void);

	protected:
		void _setTreesToLists(void);
		void _buildTreeList(void);
		void _setDefaultRatios(void);
		void _setDefaultNumTrees(void);
		void _setDefaultPlacementAllowed(void);

		afx_msg void _updateTreeWeights(void);
		afx_msg void _updateTreeCount(void);
		afx_msg void _updateGroveMakeup(void);
		afx_msg void _updatePlacementAllowed(void);

		virtual void OnOK();
		virtual void OnClose();
	DECLARE_MESSAGE_MAP()
};

extern GroveOptions *TheGroveOptions;

#endif