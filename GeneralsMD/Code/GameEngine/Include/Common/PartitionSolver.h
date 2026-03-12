// FILE: Statistics.h 
/*---------------------------------------------------------------------------*/
/* EA Pacific                                                                */
/* Confidential Information	                                                 */
/* Copyright (C) 2001 - All Rights Reserved                                  */
/* DO NOT DISTRIBUTE                                                         */
/*---------------------------------------------------------------------------*/
/* Project:    RTS3                                                          */
/* File name:  PartitionSolver.h                                             */
/* Created:    John K. McDonald, Jr., 4/2/2002                               */
/* Desc:       General purpose partition problem-solver											 */
/* Revision History:                                                         */
/*		4/12/2002 : Initial creation                                           */
/*---------------------------------------------------------------------------*/

#pragma once
#ifndef _H_PARTITIONSOLVER_
#define _H_PARTITIONSOLVER_

// INCLUDES ///////////////////////////////////////////////////////////////////

// DEFINES ////////////////////////////////////////////////////////////////////
// TYPE DEFINES ///////////////////////////////////////////////////////////////
// FORWARD DECLARATIONS ///////////////////////////////////////////////////////

typedef std::pair<ObjectID, UnsignedInt> PairObjectIDAndUInt;
typedef std::pair<ObjectID, ObjectID> PairObjectID;

// the first unsigned int is the identifying nugget, while the second is the size of the nugget
typedef std::vector<PairObjectIDAndUInt> EntriesVec;

// the first unsigned int is the identifying nugget, while the second is the size of the hole we get to fill
typedef std::vector<PairObjectIDAndUInt> SpacesVec;

// the first ObjectID is the id of the entry, while the second is the id of the hole
typedef std::vector<PairObjectID> SolutionVec;

enum SolutionType
{
	PREFER_FAST_SOLUTION = 0,
	PREFER_CORRECT_SOLUTION = 0x7FFFFFFF
};

class PartitionSolver
{
	protected:
		SolutionType m_howToSolve;
		EntriesVec m_data;
		SpacesVec m_spacesForData;

		SolutionVec m_currentSolution;
		UnsignedInt m_currentSolutionLeftovers;
		SolutionVec m_bestSolution;

	public:
		PartitionSolver(const EntriesVec& elements, const SpacesVec& spaces, SolutionType solveHow);
		
		// Solve could potentially take a LONG TIME (as in NEVER complete). This problem is NP-complete.
		void solve(void);
		const SolutionVec& getSolution( void ) const;
};

#endif /* _H_PARTITIONSOLVER_ */
