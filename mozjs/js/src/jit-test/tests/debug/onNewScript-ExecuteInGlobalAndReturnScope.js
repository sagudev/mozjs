// Debugger should be notified of scripts created with ExecuteInGlobalAndReturnScope.

var g = newGlobal({newCompartment: true});
var g2 = newGlobal({newCompartment: true});
var dbg = new Debugger(g, g2);
var log = '';
var canary = 42;

dbg.onNewScript = function (evalScript) {
  log += 'e';

  dbg.onNewScript = function (clonedScript) {
    log += 'c';
    clonedScript.setBreakpoint(0, {
      hit(frame) {
        log += 'b';
        assertEq(frame.script, clonedScript);
      }
    });
  };
};

dbg.onDebuggerStatement = function (frame) {
  log += 'd';
};

assertEq(log, '');
var evalScope = g.evalReturningScope("canary = 'dead'; let lex = 42; debugger; // nee", g2);
assertEq(log, 'ecbd');
assertEq(canary, 42);
assertEq(evalScope.canary, 'dead');
