// FILE: RankInfo.h ////////////////////////////////////////////////////////////////////////////////
// Author: Steven Johnson, Sep 2002
// Desc:   RankInfo descriptoins
///////////////////////////////////////////////////////////////////////////////////////////////////

#pragma once

#ifndef __RankInfo_H_
#define __RankInfo_H_

// INCLUDES ///////////////////////////////////////////////////////////////////////////////////////
#include "Common/Science.h"
#include "Common/UnicodeString.h"
#include "Common/STLTypedefs.h"

class Player;

//-------------------------------------------------------------------------------------------------
class RankInfo : public Overridable
{
	MEMORY_POOL_GLUE_WITH_USERLOOKUP_CREATE( RankInfo, "RankInfo" );
public:
	UnicodeString		m_rankName;
	Int							m_skillPointsNeeded;
	Int							m_sciencePurchasePointsGranted;
	ScienceVec			m_sciencesGranted;
};
//EMPTY_DTOR(RankInfo)

//-------------------------------------------------------------------------------------------------
class RankInfoStore : public SubsystemInterface
{
public:
	virtual ~RankInfoStore();

public:
	void init();
	void reset();
	void update() { }

	Int getRankLevelCount() const;

	// note that level is 1...n, NOT 0...n-1
	const RankInfo* getRankInfo(Int level) const;

	static void friend_parseRankDefinition(INI* ini);

private:

	typedef std::vector<RankInfo*> RankInfoVec;
	RankInfoVec m_rankInfos;
};

extern RankInfoStore* TheRankInfoStore;


#endif // __RankInfo_H_

