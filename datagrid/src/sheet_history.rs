// SPDX-License-Identifier: AGPL-3.0-or-later

use ranger::prelude::*;
use std::cell::RefCell;
use std::rc::Rc;

use crate::rgr::OfficeHistoryRules;
use crate::spreadsheet_model::SpreadsheetUndoOp;

/// The sheet's undo history: the operations to undo, the ones to redo, and the
/// transaction each belongs to, so one undo takes back a whole paste.
///
/// This is OfficeHistory (office/editor/OfficeHistory.rgr) for
/// SpreadsheetUndoOp, with the same method names and the same rules: the
/// transaction rules are OfficeHistoryRules itself. It is a struct of its own
/// because OfficeHistory is a generic .rgr class, and the Rust module cargo
/// builds from the .rgr closure does not hold a generic class; when it does,
/// this file can become `OfficeHistory<Rc<RefCell<SpreadsheetUndoOp>>>`.
pub struct SheetHistory {
    pub limit: i64,
    pub txDepth: i64,
    pub txCounter: i64,
    pub curTx: i64,
    pub ops: Vec<Rc<RefCell<SpreadsheetUndoOp>>>,
    pub txIds: Vec<i64>,
    pub redoOps: Vec<Rc<RefCell<SpreadsheetUndoOp>>>,
    pub redoTxIds: Vec<i64>,
}

impl SheetHistory {
    pub fn new() -> Self {
        Self {
            limit: 200,
            txDepth: 0,
            txCounter: 0,
            curTx: 0,
            ops: Vec::new(),
            txIds: Vec::new(),
            redoOps: Vec::new(),
            redoTxIds: Vec::new(),
        }
    }
    pub fn reset(&mut self) {
        self.txDepth = 0;
        self.curTx = 0;
        self.ops.clear();
        self.txIds.clear();
        self.redoOps.clear();
        self.redoTxIds.clear();
    }
    /// Opens a transaction; they nest, and only the outermost one numbers it.
    pub fn begin(&mut self) {
        if self.txDepth == 0 {
            self.txCounter += 1;
            self.curTx = self.txCounter;
        }
        self.txDepth += 1;
    }
    pub fn end(&mut self) {
        if self.txDepth > 0 {
            self.txDepth -= 1;
        }
        if self.txDepth == 0 {
            self.curTx = 0;
            self.trim();
        }
    }
    pub fn inTransaction(&self) -> bool {
        self.txDepth > 0
    }
    pub fn record(&mut self, op: Rc<RefCell<SpreadsheetUndoOp>>) {
        self.ops.push(op);
        self.txIds.push(self.curTx);
        self.redoOps.clear();
        self.redoTxIds.clear();
        self.trim();
    }
    pub fn depth(&self) -> i64 {
        self.ops.len() as i64
    }
    pub fn redoDepth(&self) -> i64 {
        self.redoOps.len() as i64
    }
    pub fn canUndo(&self) -> bool {
        !self.ops.is_empty()
    }
    pub fn canRedo(&self) -> bool {
        !self.redoOps.is_empty()
    }
    /// How many operations the next undo takes back: its whole transaction.
    pub fn undoSpan(&self) -> i64 {
        OfficeHistoryRules::span_at(&self.txIds, (self.txIds.len() as i64) - 1)
    }
    pub fn redoSpan(&self) -> i64 {
        OfficeHistoryRules::span_at(&self.redoTxIds, (self.redoTxIds.len() as i64) - 1)
    }
    /// Drops the oldest operations past `limit`, never half a transaction,
    /// and never while one is open.
    fn trim(&mut self) {
        if self.txDepth > 0 {
            return;
        }
        let drop: i64 = OfficeHistoryRules::drop_count(&self.txIds, self.limit);
        let mut k: i64 = 0;
        while k < drop {
            self.ops.remove(0);
            self.txIds.remove(0);
            k += 1;
        }
    }
    pub fn peekUndo(&self) -> Rc<RefCell<SpreadsheetUndoOp>> {
        self.ops[self.ops.len() - 1].clone()
    }
    pub fn peekRedo(&self) -> Rc<RefCell<SpreadsheetUndoOp>> {
        self.redoOps[self.redoOps.len() - 1].clone()
    }
    pub fn moveUndoToRedo(&mut self) {
        let top: usize = self.ops.len() - 1;
        self.redoOps.push(self.ops[top].clone());
        self.redoTxIds.push(self.txIds[top]);
        self.ops.remove(top);
        self.txIds.remove(top);
    }
    pub fn moveRedoToUndo(&mut self) {
        let top: usize = self.redoOps.len() - 1;
        self.ops.push(self.redoOps[top].clone());
        self.txIds.push(self.redoTxIds[top]);
        self.redoOps.remove(top);
        self.redoTxIds.remove(top);
    }
}
