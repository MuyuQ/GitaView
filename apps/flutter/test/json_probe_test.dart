import 'dart:convert';
import 'package:flutter_test/flutter_test.dart';
import 'package:gitaview/contracts/contracts.dart';

void main() {
  test('probe: SettingsSaveInput json shape', () {
    final input = SettingsSaveInput(
      expectedRevision: 2,
      settings: SettingsEditable(
        groups: const [GroupSettings(id: 'g1', name: '工作')],
        refresh: const RefreshPolicy(enabled: true, intervalMinutes: 15),
        repoAliases: const [RepoAliasAssignment(repoId: 'repo-1', alias: '文档站')],
      ),
    );
    // ignore: avoid_print
    print('PROBE ${jsonEncode(input.toJson())}');
  });
}
